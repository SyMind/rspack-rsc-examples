import { Compiler } from '@rspack/core';
import { getModuleBuildInfo } from './module-build-info';

const PLGUIN_NAME = "ReactServerPlugin";

class ReactServerPlugin {
    apply(compiler: Compiler) {
        compiler.hooks.finishMake.tap(PLGUIN_NAME, compilation => {
            for (const module of compilation.modules) {
                const buildInfo = getModuleBuildInfo(module);
                if (buildInfo.type === "client") {
                    // compilation.addInclude();
                }
            }
        });
    }
}

export default ReactServerPlugin;
