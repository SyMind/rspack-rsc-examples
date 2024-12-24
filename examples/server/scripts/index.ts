import { fork } from "node:child_process";
import { resolve } from "node:path";
import { rspack, Compiler, experiments } from "@rspack/core";
import ReactServerPlugin from './react-server-plugin';

const isDev = process.env.NODE_ENV === "development";

class BootPlugin {
  #booted = false;

  apply(compiler: Compiler) {
    compiler.hooks.done.tap("BootPlugin", (stats) => {
      if (this.#booted || stats.hasErrors()) {
        return;
      }

      fork("./dist/main.js", [], {
        stdio: "inherit",
        cwd: import.meta.dirname,
      });
      this.#booted = true;
    });
  }
}

const swcLoader = (isReactServerLayer: boolean) => ({
  loader: "builtin:swc-loader",
  options: {
    jsc: {
      parser: {
        syntax: "typescript",
        tsx: true,
      },
      transform: {
        react: {
          runtime: 'automatic',
        },
      },
      experimental: {
        plugins: [["swc-plugin-react-server", {
          isReactServerLayer: true
        }]]
      }
    },
  },
});

const serverCompiler = rspack({
  mode: isDev ? "development" : "production",
  target: "node",
  entry: ["@rspack/core/hot/poll?100", "./src/index.ts"],
  resolve: {
    extensions: ["...", ".ts", ".tsx", ".jsx"],
  },
  externals: {
    express: "commonjs express"
  },
  devtool: false,
  module: {
    rules: [
      {
        test: /react-server\.tsx$/,
        layer: "react-server",
      },
      {
        issuerLayer: "react-server",
        resolve: {
          conditionNames: ["react-server", "..."],
        }
      },
      {
        issuerLayer: "react-server",
        loader: resolve(__dirname, "react-server-loader.ts"),
      },
      {
        test: /\.(jsx?|tsx?)$/,
        issuerLayer: "react-server",
        use: [
          swcLoader(true),
        ],
      },
      {
        test: /\.(jsx?|tsx?)$/,
        use: [
          swcLoader(false),
        ],
      },
    ],
  },
  experiments: {
    layers: true,
  },
  plugins: [
    new rspack.HotModuleReplacementPlugin(),
    new ReactServerPlugin(),
    new BootPlugin(),
  ],
});

serverCompiler.watch({}, (err, stats) => {
  if (err) {
    throw err;
  }
  console.log(stats?.toString({ colors: true }));
});
