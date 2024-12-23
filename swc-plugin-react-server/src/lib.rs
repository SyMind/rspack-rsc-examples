mod cjs_finder;
mod react_server_components;

use serde::Deserialize;
use swc_core::ecma::visit::{visit_mut_pass, VisitMutWith};
use swc_core::ecma::{ast::Program, visit::VisitMut};
use swc_core::plugin::metadata::TransformPluginMetadataContextKind;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

use react_server_components::server_components;

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransformConfig {
    #[serde(default)]
    pub is_react_server_layer: bool,
}

pub struct TransformVisitor;

impl VisitMut for TransformVisitor {
    // Implement necessary visit_mut_* methods for actual custom transform.
    // A comprehensive list of possible visitor methods can be found here:
    // https://rustdoc.swc.rs/swc_ecma_visit/trait.VisitMut.html
}

/// An example plugin function with macro support.
/// `plugin_transform` macro interop pointers into deserialized structs, as well
/// as returning ptr back to host.
///
/// It is possible to opt out from macro by writing transform fn manually
/// if plugin need to handle low-level ptr directly via
/// `__transform_plugin_process_impl(
///     ast_ptr: *const u8, ast_ptr_len: i32,
///     unresolved_mark: u32, should_enable_comments_proxy: i32) ->
///     i32 /*  0 for success, fail otherwise.
///             Note this is only for internal pointer interop result,
///             not actual transform result */`
///
/// This requires manual handling of serialization / deserialization from ptrs.
/// Refer swc_plugin_macro to see how does it work internally.
#[plugin_transform]
pub fn process_transform(
    mut program: Program,
    metadata: TransformPluginProgramMetadata,
) -> Program {
    let config = match metadata.get_transform_plugin_config() {
        Some(config) => serde_json::from_str::<TransformConfig>(&config)
            .expect("invalid config for swc-plugin-react-server"),
        None => TransformConfig::default(),
    };

    let filepath = metadata
        .get_context(&TransformPluginMetadataContextKind::Filename)
        .unwrap_or_default();

    let server_components_visitor =
        server_components(filepath, config.is_react_server_layer, metadata.comments);
    program.visit_mut_with(&mut visit_mut_pass(server_components_visitor));
    program
}
