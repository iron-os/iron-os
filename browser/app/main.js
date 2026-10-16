// Iron OS kiosk browser (prototype)
//
// Replaces the chromium package and its MV2 extension. The service still
// sends the url over its websocket, but the page is loaded as the top level
// document instead of in an iframe, so no headers need to be stripped.
const { app, BrowserWindow, session } = require('electron');
const fs = require('fs');
const path = require('path');

const config = JSON.parse(
	fs.readFileSync(path.join(__dirname, 'config.json'), 'utf8')
);

const LOADING_PAGE =
	'data:text/html,<body style="background:%23000;color:%23fff;' +
	'font-family:sans-serif">Service: Loading Page</body>';

app.commandLine.appendSwitch('disable-pinch');
app.commandLine.appendSwitch('overscroll-history-navigation', '0');
app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');

let win = null;
let currentUrl = null;
let rendererOk = true;

// whitelist entries are like the old extension: "host" matches exactly,
// "*suffix" matches the end of the host, "*" matches everything
function hostAllowed(hostname) {
	if (hostname === '127.0.0.1' || hostname === 'localhost')
		return true;

	return config.whitelist.some(w => {
		if (w.startsWith('*'))
			return hostname.endsWith(w.slice(1));
		return hostname === w;
	});
}

function setupSession() {
	const ses = session.defaultSession;

	ses.webRequest.onBeforeRequest((details, cb) => {
		let url;
		try {
			url = new URL(details.url);
		} catch (e) {
			return cb({ cancel: true });
		}

		if (!['http:', 'https:', 'ws:', 'wss:'].includes(url.protocol))
			return cb({});

		const allowed = hostAllowed(url.hostname);
		if (!allowed)
			console.log('blocked', details.url);
		cb({ cancel: !allowed });
	});

	// kiosk: no permission prompts, only allow what a display needs
	ses.setPermissionRequestHandler((wc, permission, cb) => {
		cb(['fullscreen', 'clipboard-sanitized-write'].includes(permission));
	});
}

function openPage(url) {
	currentUrl = url;
	console.log('open page', url);
	win.loadURL(url).catch(e => console.log('load failed', e.message));
}

function createWindow() {
	win = new BrowserWindow({
		kiosk: true,
		frame: false,
		backgroundColor: '#000000',
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
		},
	});

	const wc = win.webContents;
	wc.setVisualZoomLevelLimits(1, 1);
	wc.setWindowOpenHandler(() => ({ action: 'deny' }));

	// same as reloadOnError in the old extension
	wc.on('did-fail-load', (e, code, desc, url, isMainFrame) => {
		// -3 is ERR_ABORTED, happens when a new url is loaded,
		// -20 ERR_BLOCKED_BY_CLIENT is the whitelist, retrying won't help
		if (!isMainFrame || code === -3 || code === -20 || !currentUrl)
			return;
		console.log('load failed', code, desc, url);
		setTimeout(() => openPage(currentUrl), 1000);
	});

	wc.on('render-process-gone', (e, details) => {
		console.log('renderer gone', details.reason);
		if (currentUrl)
			setTimeout(() => openPage(currentUrl), 1000);
	});
	wc.on('unresponsive', () => rendererOk = false);
	wc.on('responsive', () => rendererOk = true);

	win.loadURL(LOADING_PAGE);
}

// talks to the service like the old landing page did: receives urls and
// sends StillAlive so the service watchdog restarts us if we hang
function connectService() {
	const ws = new WebSocket(config.serviceStream);
	let interval = null;

	const alive = () => {
		if (rendererOk && ws.readyState === WebSocket.OPEN)
			ws.send('StillAlive');
	};

	ws.addEventListener('open', () => {
		console.log('connected to service');
		alive();
		interval = setInterval(alive, 5000);
	});
	ws.addEventListener('message', msg => {
		if (typeof msg.data === 'string')
			openPage(msg.data);
	});
	ws.addEventListener('close', () => {
		clearInterval(interval);
		setTimeout(connectService, 2000);
	});
	ws.addEventListener('error', () => {});
}

// look like a normal chrome, some sites check the user agent
app.userAgentFallback = app.userAgentFallback
	.replace(/ iron-os-browser\/\S+/, '')
	.replace(/ Electron\/\S+/, '');

app.whenReady().then(() => {
	setupSession();
	createWindow();
	connectService();
});

app.on('window-all-closed', () => app.quit());
