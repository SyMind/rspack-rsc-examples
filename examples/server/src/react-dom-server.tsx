import { Readable } from "node:stream";
import type { ReadableStream as NodeReadableStream } from "stream/web";

import {
  type Request as ExpressRequest,
  type Response as ExpressResponse,
} from "express";
import { injectRSCPayload } from "rsc-html-stream/server";
// @ts-ignore
import { renderToReadableStream } from "react-dom/server.edge";
import ReactClient, { ReactElement } from "react";
import { createFromReadableStream } from "react-server-dom-webpack/client.edge";

import * as ReactServer from "./react-server";

export async function render(req: ExpressRequest, res: ExpressResponse) {
  const stream = ReactServer.render();
  if (req.accepts("text/html")) {
    res.setHeader("Content-Type", "text/html");

    // Use client react to render the RSC payload to HTML.
    const [s1, s2] = stream.tee();
    const data = createFromReadableStream<ReactElement>(s1, {
      serverConsumerManifest: {
        moduleMap: {},
        moduleLoading: null,
        serverModuleMap: null
      }
    });
    function Content() {
      return ReactClient.use(data);
    }

    const htmlStream = await renderToReadableStream(<Content />);
    const response = htmlStream.pipeThrough(injectRSCPayload(s2));
    Readable.fromWeb(response as NodeReadableStream).pipe(res);
  } else {
    res.set("Content-Type", "text/x-component");
    Readable.fromWeb(stream as NodeReadableStream).pipe(res);
  }
}
