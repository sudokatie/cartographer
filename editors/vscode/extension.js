// Cartographer VS Code Extension
// Provides LSP client for Cartographer code analysis

const vscode = require('vscode');
const {
    LanguageClient,
    TransportKind,
    LanguageClientOptions,
    ServerOptions
} = require('vscode-languageclient/node');

let client = null;
let outputChannel = null;

/**
 * Activate the extension
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
    outputChannel = vscode.window.createOutputChannel('Cartographer');
    outputChannel.appendLine('Cartographer extension activating...');

    // Start the language server
    startLanguageServer(context);

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('cartographer.analyzeWorkspace', analyzeWorkspace),
        vscode.commands.registerCommand('cartographer.restartServer', () => restartServer(context))
    );

    // Watch for configuration changes
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('cartographer')) {
                onConfigurationChanged(context);
            }
        })
    );

    outputChannel.appendLine('Cartographer extension activated');
}

/**
 * Start the language server
 * @param {vscode.ExtensionContext} context
 */
function startLanguageServer(context) {
    const config = vscode.workspace.getConfiguration('cartographer');
    const binaryPath = config.get('binaryPath', 'cartographer');

    // Server options - run cartographer lsp command
    const serverOptions = {
        run: {
            command: binaryPath,
            args: ['lsp'],
            transport: TransportKind.stdio
        },
        debug: {
            command: binaryPath,
            args: ['lsp'],
            transport: TransportKind.stdio
        }
    };

    // Client options
    const clientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'rust' },
            { scheme: 'file', language: 'python' },
            { scheme: 'file', language: 'javascript' },
            { scheme: 'file', language: 'typescript' },
            { scheme: 'file', language: 'javascriptreact' },
            { scheme: 'file', language: 'typescriptreact' },
            { scheme: 'file', language: 'go' },
            { scheme: 'file', language: 'java' },
            { scheme: 'file', language: 'c' },
            { scheme: 'file', language: 'cpp' }
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{rs,py,js,ts,jsx,tsx,go,java,c,cpp,h,hpp}')
        },
        outputChannel: outputChannel,
        traceOutputChannel: outputChannel
    };

    // Create and start the client
    client = new LanguageClient(
        'cartographer',
        'Cartographer Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client
    client.start().then(() => {
        outputChannel.appendLine('Cartographer LSP server started');
    }).catch(err => {
        outputChannel.appendLine(`Failed to start LSP server: ${err.message}`);
        vscode.window.showErrorMessage(
            `Cartographer: Failed to start language server. Is 'cartographer' installed and in PATH? Error: ${err.message}`
        );
    });

    context.subscriptions.push(client);
}

/**
 * Stop the language server
 */
async function stopLanguageServer() {
    if (client) {
        await client.stop();
        client = null;
    }
}

/**
 * Restart the language server
 * @param {vscode.ExtensionContext} context
 */
async function restartServer(context) {
    outputChannel.appendLine('Restarting Cartographer LSP server...');
    await stopLanguageServer();
    startLanguageServer(context);
    vscode.window.showInformationMessage('Cartographer: Language server restarted');
}

/**
 * Handle configuration changes
 * @param {vscode.ExtensionContext} context
 */
async function onConfigurationChanged(context) {
    // Restart server if binary path changed
    const config = vscode.workspace.getConfiguration('cartographer');
    const newBinaryPath = config.get('binaryPath', 'cartographer');

    outputChannel.appendLine(`Configuration changed, binary path: ${newBinaryPath}`);

    // For now, restart on any config change
    await restartServer(context);
}

/**
 * Analyze workspace command
 */
async function analyzeWorkspace() {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders || workspaceFolders.length === 0) {
        vscode.window.showWarningMessage('Cartographer: No workspace folder open');
        return;
    }

    const config = vscode.workspace.getConfiguration('cartographer');
    const binaryPath = config.get('binaryPath', 'cartographer');
    const excludes = config.get('analysis.exclude', []);

    const workspacePath = workspaceFolders[0].uri.fsPath;

    outputChannel.appendLine(`Analyzing workspace: ${workspacePath}`);

    // Run cartographer analyze command
    const terminal = vscode.window.createTerminal('Cartographer');

    let command = `${binaryPath} analyze "${workspacePath}"`;
    for (const exclude of excludes) {
        command += ` --exclude "${exclude}"`;
    }

    terminal.sendText(command);
    terminal.show();

    vscode.window.showInformationMessage('Cartographer: Analysis started. Check the terminal for progress.');
}

/**
 * Deactivate the extension
 */
function deactivate() {
    if (outputChannel) {
        outputChannel.appendLine('Cartographer extension deactivating...');
    }
    return stopLanguageServer();
}

module.exports = {
    activate,
    deactivate
};
