import express from "express";
import * as ReactDomServer from './react-dom-server';

const app = express();

app.use(function (req, res, next) {
  res.setHeader("Access-Control-Allow-Methods", "GET,HEAD,POST");
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Headers", "rsc-action");
  next();
});

app.get("/", (req, res) => {
  ReactDomServer.render(req, res);
});

const server = app.listen(3001);
console.log('Server listening on port 3001');

// Restart the server when it changes.
if (module.hot) {
  module.hot.dispose(() => {
    server.close();
  });
  module.hot.accept();
}