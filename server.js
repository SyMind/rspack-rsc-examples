import http from 'http';
import path from 'path';
import express from 'express';
import rspack from '@rspack/core';
import webpackDevMiddleware from 'webpack-dev-middleware';
import webpackHotMiddleware from 'webpack-hot-middleware';
import ReactRefreshPlugin from '@rspack/plugin-react-refresh';

// Target browsers, see: https://github.com/browserslist/browserslist
const browserTargets = ["last 2 versions", "> 0.2%", "not dead", "Firefox ESR"];
// Target Node.js LTS version for server bundle
const nodeTargets = ["node 22"];

function jsRule(targets) {
    return {
        test: /\.jsx?$/,
        use: [
            {
                loader: "builtin:swc-loader",
                options: {
                    jsc: {
                        parser: {
                            syntax: "ecmascript",
                            jsx: true,
                        },
                        transform: {
                            react: {
                                runtime: "automatic",
                            },
                        },
                        experimental: {
                            keepImportAttributes: true,
                        },
                    },
                    env: { targets },
                    rspackExperiments: {
                        reactServerComponents: true,
                    },
                },
            },
        ],
    }
};

function tsRule(targets) {
    return {
        test: /\.tsx?$/,
        use: [
            {
                loader: "builtin:swc-loader",
                options: {
                    jsc: {
                        parser: {
                            syntax: "typescript",
                            tsx: true,
                        },
                        transform: {
                            react: {
                                runtime: "automatic",
                            },
                        },
                        experimental: {
                            keepImportAttributes: true,
                        },
                    },
                    env: { targets },
                    rspackExperiments: {
                        reactServerComponents: true,
                    },
                },
            },
        ],
    }
};

function cssRule() {
    return {
        test: /\.css$/i,
        type: 'css/auto',
    }
};

const { createPlugins, Layers } = rspack.experiments.rsc;
const { ServerPlugin, ClientPlugin } = createPlugins();

const SSR_ENTRY = path.resolve(import.meta.dirname, "src/framework/entry.ssr.tsx");
const RSC_ENTRY = path.resolve(import.meta.dirname, "src/framework/entry.rsc.tsx");

const rspackConfig = [
    {
        name: 'client',
        mode: 'development',
        target: 'web',
        context: import.meta.dirname,
        entry: [
            // Add the client which connects to our middleware
            // You can use full urls like 'webpack-hot-middleware/client?path=http://localhost:3000/__webpack_hmr'
            // useful if you run your app from another point like django
            'webpack-hot-middleware/client?path=/__rspack_hmr&timeout=20000',
            // And then the actual application
            './src/framework/entry.client.tsx',
        ],
        resolve: {
            extensions: ["...", ".ts", ".tsx", ".jsx"],
        },
        output: {
            path: path.join(import.meta.dirname, 'dist/static'),
            publicPath: 'static/',
        },
        devtool: 'source-map',
        module: {
            rules: [
                cssRule(),
                jsRule(browserTargets),
                tsRule(browserTargets),
            ]
        },
        plugins: [
            new ClientPlugin(),
            new rspack.HotModuleReplacementPlugin(),
            new ReactRefreshPlugin(),
        ],
    },
    {
        name: 'server',
        mode: 'development',
        target: 'node',
        context: import.meta.dirname,
        entry: './src/framework/entry.rsc.tsx',
        resolve: {
            extensions: ["...", ".ts", ".tsx", ".jsx"],
        },
        output: {
            path: path.join(import.meta.dirname, 'dist'),
            module: true,
            chunkFormat: 'module',
            chunkLoading: 'import',
            library: {
                type: 'module',
            },
        },
        devtool: false,
        module: {
            rules: [
                cssRule(),
                jsRule(nodeTargets),
                tsRule(nodeTargets),
                // react server components layers
                {
                    resource: SSR_ENTRY,
                    layer: Layers.ssr,
                },
                {
                    resource: RSC_ENTRY,
                    layer: Layers.rsc,
                    resolve: {
                        conditionNames: ["react-server", "..."],
                    },
                },
                {
                    issuerLayer: Layers.rsc,
                    exclude: SSR_ENTRY,
                    resolve: {
                        conditionNames: ["react-server", "..."],
                    },
                },
            ]
        },
        plugins: [
            new ServerPlugin({
                onServerComponentChanges() {
                    console.log("[RSC] server component changes detected, restarting server...");
                }
            }),
        ],
    }
];
const compiler = rspack(rspackConfig);

const app = express();

app.use(
    webpackDevMiddleware(compiler, {
        writeToDisk: true,
    })
);

app.use(
    webpackHotMiddleware(compiler.compilers[0], {
        log: console.log,
        path: '/__rspack_hmr',
        heartbeat: 10 * 1000,
    })
);

app.use(async (req, res, next) => {
    const mod = await import('./dist/main.mjs');
    await mod.default.nodeHandler(req, res, next);
});

const server = http.createServer(app);
server.listen(process.env.PORT || 1616, "localhost", function () {
    console.log('Listening on %j', server.address());
});
