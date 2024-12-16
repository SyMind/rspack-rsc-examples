import type { LoaderContext } from '@rspack/core';
import { setModuleBuildInfo } from './module-build-info';

export default function reactServerLoader(this: LoaderContext<void>, source: string) {
    if (source.startsWith('"use client";')) {
        setModuleBuildInfo(this._module, { type: "client" });

        return `
        import { registerClientReference } from "react-server-dom-webpack/server";
        export default registerClientReference(
            function() {{ throw new Error("error"); }},
            ${JSON.stringify(this.resource)},
            "default",
        );`;
    }

    return source;
}
