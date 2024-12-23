use std::{collections::HashMap, rc::Rc, sync::Arc};

use lazy_static::lazy_static;
use regex::Regex;
use swc_core::{
    common::{
        comments::{Comment, CommentKind, Comments},
        errors::HANDLER,
        util::take::Take,
        Span, DUMMY_SP,
    },
    ecma::{
        ast::*,
        atoms::{js_word, JsWord},
        utils::{prepend_stmts, quote_ident, quote_str, ExprFactory},
        visit::{noop_visit_mut_type, noop_visit_type, Visit, VisitMut, VisitMutWith, VisitWith},
    },
};

use super::cjs_finder::contains_cjs;

/// A visitor that transforms given module to use module proxy if it's a React
/// server component.
/// **NOTE** Turbopack uses ClientDirectiveTransformer for the
/// same purpose, so does not run this transform.
struct ReactServerComponents<C: Comments> {
    is_react_server_layer: bool,
    filepath: Arc<str>,
    comments: C,
    directive_import_collection: Option<(bool, bool, RcVec<ModuleImports>, RcVec<String>)>,
}

#[derive(Clone, Debug)]
struct ModuleImports {
    source: (JsWord, Span),
    specifiers: Vec<(JsWord, Span)>,
}

enum RSCErrorKind {
    /// When `use client` and `use server` are in the same file.
    /// It's not possible to have both directives in the same file.
    RedundantDirectives(Span),
    NextRscErrServerImport((String, Span)),
    NextRscErrClientDirective(Span),
    NextRscErrReactApi((String, Span)),
}

impl<C: Comments> VisitMut for ReactServerComponents<C> {
    noop_visit_mut_type!();

    fn visit_mut_module(&mut self, module: &mut Module) {
        // Run the validator first to assert, collect directives and imports.
        let mut validator =
            ReactServerComponentValidator::new(self.is_react_server_layer, self.filepath.clone());

        module.visit_with(&mut validator);
        self.directive_import_collection = validator.directive_import_collection;

        let is_client_entry = self
            .directive_import_collection
            .as_ref()
            .expect("directive_import_collection must be set")
            .0;

        self.remove_top_level_directive(module);

        let is_cjs = contains_cjs(module);

        if self.is_react_server_layer {
            if is_client_entry {
                self.to_module_ref(module, is_cjs);
                return;
            }
        } else if is_client_entry {
            self.prepend_comment_node(module, is_cjs);
        }
        module.visit_mut_children_with(self)
    }
}

impl<C: Comments> ReactServerComponents<C> {
    /// removes specific directive from the AST.
    fn remove_top_level_directive(&mut self, module: &mut Module) {
        let _ = &module.body.retain(|item| {
            if let ModuleItem::Stmt(stmt) = item {
                if let Some(expr_stmt) = stmt.as_expr() {
                    if let Expr::Lit(Lit::Str(Str { value, .. })) = &*expr_stmt.expr {
                        if &**value == "use client" {
                            // Remove the directive.
                            return false;
                        }
                    }
                }
            }
            true
        });
    }

    // Convert the client module to the module reference code and add a special
    // comment to the top of the file.
    fn to_module_ref(&self, module: &mut Module, is_cjs: bool) {
        // Clear all the statements and module declarations.
        module.body.clear();

        let proxy_ident = quote_ident!("createProxy");
        let filepath = quote_str!(&*self.filepath);

        prepend_stmts(
            &mut module.body,
            vec![
                ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(VarDecl {
                    span: DUMMY_SP,
                    kind: VarDeclKind::Const,
                    decls: vec![VarDeclarator {
                        span: DUMMY_SP,
                        name: Pat::Object(ObjectPat {
                            span: DUMMY_SP,
                            props: vec![ObjectPatProp::Assign(AssignPatProp {
                                span: DUMMY_SP,
                                key: proxy_ident.into(),
                                value: None,
                            })],
                            optional: false,
                            type_ann: None,
                        }),
                        init: Some(Box::new(Expr::Call(CallExpr {
                            span: DUMMY_SP,
                            callee: quote_ident!("require").as_callee(),
                            args: vec![quote_str!("private-next-rsc-mod-ref-proxy").as_arg()],
                            ..Default::default()
                        }))),
                        definite: false,
                    }],
                    ..Default::default()
                })))),
                ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                    span: DUMMY_SP,
                    expr: Box::new(Expr::Assign(AssignExpr {
                        span: DUMMY_SP,
                        left: MemberExpr {
                            span: DUMMY_SP,
                            obj: Box::new(Expr::Ident(quote_ident!("module").into())),
                            prop: MemberProp::Ident(quote_ident!("exports")),
                        }
                        .into(),
                        op: op!("="),
                        right: Box::new(Expr::Call(CallExpr {
                            span: DUMMY_SP,
                            callee: quote_ident!("createProxy").as_callee(),
                            args: vec![filepath.as_arg()],
                            ..Default::default()
                        })),
                    })),
                })),
            ]
            .into_iter(),
        );

        self.prepend_comment_node(module, is_cjs);
    }

    fn prepend_comment_node(&self, module: &Module, is_cjs: bool) {
        let export_names = &self
            .directive_import_collection
            .as_ref()
            .expect("directive_import_collection must be set")
            .3;

        // Prepend a special comment to the top of the file that contains
        // module export names and the detected module type.
        self.comments.add_leading(
            module.span.lo,
            Comment {
                span: DUMMY_SP,
                kind: CommentKind::Block,
                text: format!(
                    " __next_internal_client_entry_do_not_use__ {} {} ",
                    export_names.join(","),
                    if is_cjs { "cjs" } else { "auto" }
                )
                .into(),
            },
        );
    }
}

/// Consolidated place to parse, generate error messages for the RSC parsing
/// errors.
fn report_error(error_kind: RSCErrorKind) {
    let (msg, span) = match error_kind {
        RSCErrorKind::RedundantDirectives(span) => (
            "It's not possible to have both `use client` and `use server` directives in the \
             same file."
                .to_string(),
            span,
        ),
        RSCErrorKind::NextRscErrClientDirective(span) => (
            "The \"use client\" directive must be placed before other expressions. Move it to \
             the top of the file to resolve this issue."
                .to_string(),
            span,
        ),
        RSCErrorKind::NextRscErrServerImport((source, span)) => {
            let msg = match source.as_str() {
                // If importing "react-dom/server", we should show a different error.
                "react-dom/server" => "You're importing a component that imports react-dom/server. To fix it, render or return the content directly as a Server Component instead for perf and security.".to_string(),
                _ => format!(r#"You're importing a component that imports {source}. It only works in a Client Component but none of its parents are marked with "use client", so they're Server Components by default.\n\n"#)
            };

            (msg, span)
        }
        RSCErrorKind::NextRscErrReactApi((source, span)) => {
            let msg = if source == "Component" {
                "You’re importing a class component. It only works in a Client Component but none of its parents are marked with \"use client\", so they're Server Components by default.\n\n".to_string()
            } else {
                format!("You're importing a component that needs `{source}`. This React hook only works in a client component. To fix, mark the file (or its parent) with the `\"use client\"` directive.\n\n")
            };

            (msg, span)
        }
    };

    HANDLER.with(|handler| handler.struct_span_err(span, msg.as_str()).emit())
}

/// Collects top level directives and imports
fn collect_top_level_directives_and_imports(
    module: &Module,
) -> (bool, bool, Vec<ModuleImports>, Vec<String>) {
    let mut imports: Vec<ModuleImports> = vec![];
    let mut finished_directives = false;
    let mut is_client_entry = false;
    let mut is_action_file = false;

    let mut export_names = vec![];

    let _ = &module.body.iter().for_each(|item| {
        match item {
            ModuleItem::Stmt(stmt) => {
                if !stmt.is_expr() {
                    // Not an expression.
                    finished_directives = true;
                }

                match stmt.as_expr() {
                    Some(expr_stmt) => {
                        match &*expr_stmt.expr {
                            Expr::Lit(Lit::Str(Str { value, .. })) => {
                                if &**value == "use client" {
                                    if !finished_directives {
                                        is_client_entry = true;

                                        if is_action_file {
                                            report_error(RSCErrorKind::RedundantDirectives(
                                                expr_stmt.span,
                                            ));
                                        }
                                    } else {
                                        report_error(RSCErrorKind::NextRscErrClientDirective(
                                            expr_stmt.span,
                                        ));
                                    }
                                } else if &**value == "use server" && !finished_directives {
                                    is_action_file = true;

                                    if is_client_entry {
                                        report_error(RSCErrorKind::RedundantDirectives(
                                            expr_stmt.span,
                                        ));
                                    }
                                }
                            }
                            // Match `ParenthesisExpression` which is some formatting tools
                            // usually do: ('use client'). In these case we need to throw
                            // an exception because they are not valid directives.
                            Expr::Paren(ParenExpr { expr, .. }) => {
                                finished_directives = true;
                                if let Expr::Lit(Lit::Str(Str { value, .. })) = &**expr {
                                    if &**value == "use client" {
                                        report_error(RSCErrorKind::NextRscErrClientDirective(
                                            expr_stmt.span,
                                        ));
                                    }
                                }
                            }
                            _ => {
                                // Other expression types.
                                finished_directives = true;
                            }
                        }
                    }
                    None => {
                        // Not an expression.
                        finished_directives = true;
                    }
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(
                import @ ImportDecl {
                    type_only: false, ..
                },
            )) => {
                let source = import.src.value.clone();
                let specifiers = import
                    .specifiers
                    .iter()
                    .filter(|specifier| {
                        !matches!(
                            specifier,
                            ImportSpecifier::Named(ImportNamedSpecifier {
                                is_type_only: true,
                                ..
                            })
                        )
                    })
                    .map(|specifier| match specifier {
                        ImportSpecifier::Named(named) => match &named.imported {
                            Some(imported) => match &imported {
                                ModuleExportName::Ident(i) => (i.to_id().0, i.span),
                                ModuleExportName::Str(s) => (s.value.clone(), s.span),
                            },
                            None => (named.local.to_id().0, named.local.span),
                        },
                        ImportSpecifier::Default(d) => (js_word!(""), d.span),
                        ImportSpecifier::Namespace(n) => ("*".into(), n.span),
                    })
                    .collect();

                imports.push(ModuleImports {
                    source: (source, import.span),
                    specifiers,
                });

                finished_directives = true;
            }
            // Collect all export names.
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(e)) => {
                for specifier in &e.specifiers {
                    export_names.push(match specifier {
                        ExportSpecifier::Default(_) => "default".to_string(),
                        ExportSpecifier::Namespace(_) => "*".to_string(),
                        ExportSpecifier::Named(named) => match &named.exported {
                            Some(exported) => match &exported {
                                ModuleExportName::Ident(i) => i.sym.to_string(),
                                ModuleExportName::Str(s) => s.value.to_string(),
                            },
                            _ => match &named.orig {
                                ModuleExportName::Ident(i) => i.sym.to_string(),
                                ModuleExportName::Str(s) => s.value.to_string(),
                            },
                        },
                    })
                }
                finished_directives = true;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                match decl {
                    Decl::Class(ClassDecl { ident, .. }) => {
                        export_names.push(ident.sym.to_string());
                    }
                    Decl::Fn(FnDecl { ident, .. }) => {
                        export_names.push(ident.sym.to_string());
                    }
                    Decl::Var(var) => {
                        for decl in &var.decls {
                            if let Pat::Ident(ident) = &decl.name {
                                export_names.push(ident.id.sym.to_string());
                            }
                        }
                    }
                    _ => {}
                }
                finished_directives = true;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(ExportDefaultDecl {
                decl: _,
                ..
            })) => {
                export_names.push("default".to_string());
                finished_directives = true;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(ExportDefaultExpr {
                expr: _,
                ..
            })) => {
                export_names.push("default".to_string());
                finished_directives = true;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(_)) => {
                export_names.push("*".to_string());
            }
            _ => {
                finished_directives = true;
            }
        }
    });

    (is_client_entry, is_action_file, imports, export_names)
}

/// A visitor to assert given module file is a valid React server component.
struct ReactServerComponentValidator {
    is_react_server_layer: bool,
    filepath: Arc<str>,
    invalid_server_imports: Vec<JsWord>,
    invalid_server_lib_apis_mapping: HashMap<&'static str, Vec<&'static str>>,
    pub directive_import_collection: Option<(bool, bool, RcVec<ModuleImports>, RcVec<String>)>,
}

// A type to workaround a clippy warning.
type RcVec<T> = Rc<Vec<T>>;

impl ReactServerComponentValidator {
    pub fn new(is_react_server_layer: bool, filepath: Arc<str>) -> Self {
        Self {
            is_react_server_layer,
            filepath,
            directive_import_collection: None,
            // react -> [apis]
            // react-dom -> [apis]
            invalid_server_lib_apis_mapping: [
                (
                    "react",
                    vec![
                        "Component",
                        "createContext",
                        "createFactory",
                        "PureComponent",
                        "useDeferredValue",
                        "useEffect",
                        "useImperativeHandle",
                        "useInsertionEffect",
                        "useLayoutEffect",
                        "useReducer",
                        "useRef",
                        "useState",
                        "useSyncExternalStore",
                        "useTransition",
                        "useOptimistic",
                        "useActionState",
                        "experimental_useOptimistic",
                    ],
                ),
                (
                    "react-dom",
                    vec![
                        "flushSync",
                        "unstable_batchedUpdates",
                        "useFormStatus",
                        "useFormState",
                    ],
                ),
            ]
            .into(),

            invalid_server_imports: vec![
                JsWord::from("react-dom/client"),
                JsWord::from("react-dom/server"),
            ],
        }
    }

    fn is_from_node_modules(&self, filepath: &str) -> bool {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"node_modules[\\/]").unwrap();
        }
        RE.is_match(filepath)
    }

    // Asserts the server lib apis
    // e.g.
    // assert_invalid_server_lib_apis("react", import)
    // assert_invalid_server_lib_apis("react-dom", import)
    fn assert_invalid_server_lib_apis(&self, import_source: String, import: &ModuleImports) {
        let invalid_apis = self
            .invalid_server_lib_apis_mapping
            .get(import_source.as_str());
        if let Some(invalid_apis) = invalid_apis {
            for specifier in &import.specifiers {
                if invalid_apis.contains(&specifier.0.as_str()) {
                    report_error(RSCErrorKind::NextRscErrReactApi((
                        specifier.0.to_string(),
                        specifier.1,
                    )));
                }
            }
        }
    }

    fn assert_server_graph(&self, imports: &[ModuleImports]) {
        // If the
        if self.is_from_node_modules(&self.filepath) {
            return;
        }
        for import in imports {
            let source = import.source.0.clone();
            let source_str = source.to_string();
            if self.invalid_server_imports.contains(&source) {
                report_error(RSCErrorKind::NextRscErrServerImport((
                    source_str.clone(),
                    import.source.1,
                )));
            }

            self.assert_invalid_server_lib_apis(source_str, import);
        }
    }
}

impl Visit for ReactServerComponentValidator {
    noop_visit_type!();

    // coerce parsed script to run validation for the context, which is still
    // required even if file is empty
    fn visit_script(&mut self, script: &swc_core::ecma::ast::Script) {
        if script.body.is_empty() {
            self.visit_module(&Module::dummy());
        }
    }

    fn visit_module(&mut self, module: &Module) {
        let (is_client_entry, is_action_file, imports, export_names) =
            collect_top_level_directives_and_imports(module);
        let imports = Rc::new(imports);
        let export_names = Rc::new(export_names);

        self.directive_import_collection = Some((
            is_client_entry,
            is_action_file,
            imports.clone(),
            export_names,
        ));

        if self.is_react_server_layer {
            if is_client_entry {
                return;
            } else {
                // Only assert server graph if file's bundle target is "server"
                self.assert_server_graph(&imports);
            }
        }

        module.visit_children_with(self);
    }
}

/// Runs react server component transform for the module proxy, as well as
/// running assertion.
pub fn server_components<C: Comments>(
    filepath: String,
    is_react_server_layer: bool,
    comments: C,
) -> impl VisitMut {
    ReactServerComponents {
        is_react_server_layer,
        comments,
        filepath: filepath.into(),
        directive_import_collection: None,
    }
}
