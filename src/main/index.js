const electron = require('electron');

console.log('process.versions.electron:', process.versions.electron);
console.log('typeof require("electron"):', typeof electron);
console.log('electron value:', electron);

const { app, BrowserWindow } = electron;

if (!app) {
  throw new Error(
    'Electron API not available. require("electron") returned: ' + String(electron)
  );
}

function createWindow() {
  const win = new BrowserWindow({ width: 400, height: 300 });
  win.loadURL('data:text/html;charset=utf-8,<h1>Electron OK</h1>');
}

app.whenReady().then(createWindow);
app.on('window-all-closed', () => app.quit());
