"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const path = require("path");
const vscode = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
function activate(context) {
    // The server is implemented in Rust: `turn lsp`
    // We expect the `turn` executable to be either in the path or we assume we're running locally via cargo
    // For local development, we'll try to use the locally built binary
    // In production, this would look for 'turn' in the user's PATH
    let serverCommand = 'cargo';
    let serverArgs = ['run', '--bin', 'turn', '--manifest-path', path.join(context.extensionPath, '..', '..', 'impl', 'Cargo.toml'), '--', 'lsp'];
    // If we're not running inside the turn repo, fallback to global `turn lsp`
    if (!require('fs').existsSync(path.join(context.extensionPath, '..', '..', 'impl', 'Cargo.toml'))) {
        serverCommand = 'turn';
        serverArgs = ['lsp'];
    }
    const run = {
        command: serverCommand,
        args: serverArgs,
        options: { env: process.env }
    };
    const serverOptions = {
        run: run,
        debug: run
    };
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'turn' }],
        synchronize: {
            // Notify the server about file changes to '.tn' files contained in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tn')
        }
    };
    client = new node_1.LanguageClient('turnLanguageServer', 'Turn Language Server', serverOptions, clientOptions);
    client.start();
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map