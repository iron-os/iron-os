
// remove disable iframe headers
chrome.webRequest.onHeadersReceived.addListener(info => {
	const headers = info.responseHeaders;

	for (let i = headers.length - 1; i >= 0; i--) {
		const name = headers[i].name.toLowerCase();
		if (name === 'x-frame-options' ||
			name === 'frame-options' ||
			name === 'content-security-policy')
			headers.splice(i, 1); // remove it
	}

	return { responseHeaders: headers };
}, {
	urls: [ '*://www.speedtest.net/*', '*://speedtest.net/*' ],
	types: [ 'sub_frame' ]
}, ['blocking', 'responseHeaders', 'extraHeaders']);


// disable
/*
chrome.webRequest.onBeforeSendHeaders.addListener(info => {
	//const headers = info.requestHeaders;
	//console.log(info);
	//return { requestHeaders: headers };
}, {
	urls: [ '<all_urls>' ],
	types: [ 'sub_frame' ]
}, [ 'blocking', 'requestHeaders', 'extraHeaders' ]);
*/

/*
only allow whitelisted urls
*/

const whitelist = [
	// main 
	'127.0.0.1:8888',
	// to test speed in debug view
	'www.speedtest.net', 'speedtest.net',
];

const speedtestBlacklist = [
	'c.amazon-adsystem.com', 'www.googletagmanager.com',
	'sb.scorecardresearch.com', 'gurgle.speedtest.net',
	'securepubads.g.doubleclick.net', 'www.google-analytics.com',
	'jogger.zdbb.net', 'fastlane.rebiconproject.com',
	'ib.adnxs.com', 'c2shb.ssp.yahoo.com',
	'stags.bluekai.com', 'adservice.google.com', 'adservice.google.ch',
	'ookla-d.openx.net'
];
chrome.webRequest.onBeforeRequest.addListener(info => {

	console.log(info.initiator, info.url);

	// allow wss initiated from speedtest
	if ('initiator' in info &&
		info.initiator === 'https://www.speedtest.net' &&
		(info.url.startsWith('wss://') || info.url.startsWith('https://')) &&
		speedtestBlacklist.indexOf(info.url.split('/')[2]) === -1)
		return { cancel: false };

	//console.log(info);
	const domain = info.url.split('/')[2];

	return {
		cancel: whitelist.indexOf(domain) === -1
	};
}, {
	urls: [ '<all_urls>' ]
}, [ 'blocking' ]);

/* Zoom

// getting zoom
const zoom = await chrome.tabs.getZoom();

// setting zoom
await chrome.tabs.setZoom(null, 1);

// listening on zoom change
chrome.tabs.onZoomChange.addListener(({ newZoomFactor, oldZoomFactor, tabId, zoomSettings }));

*/

chrome.runtime.onMessage.addListener(async (msg, sender, sendRes) => {
	if (typeof msg !== 'string')
		return;
	if (msg === 'resetZoom') {
		const r = await chrome.tabs.setZoom(null, 1);
		console.log('zoom reset', r);
	}
})