import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    Executable
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
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

    const run: Executable = {
        command: serverCommand,
        args: serverArgs,
        options: { env: process.env }
    };

    const serverOptions: ServerOptions = {
        run: run,
        debug: run
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'turn' }],
        synchronize: {
            // Notify the server about file changes to '.tn' files contained in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tn')
        }
    };

    client = new LanguageClient(
        'turnLanguageServer',
        'Turn Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
