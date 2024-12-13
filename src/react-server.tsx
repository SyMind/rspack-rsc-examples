import React from 'react';
import { renderToReadableStream } from 'react-server-dom-webpack/server.edge';
import { App } from './app';

export function render(): ReturnType<typeof renderToReadableStream> {
    const clientManifest = {};
    return renderToReadableStream(React.createElement(App), clientManifest);
}
