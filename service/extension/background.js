
// config: { whitelist: [] }
import config from './config.js';

function main() {
	allowIframes();
	blockRequests();
	reloadOnError();
	// handleConnection();
}

// remove disable iframe headers
function allowIframes() {

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
		urls: [ '<all_urls>' ],
		types: [ 'sub_frame' ]
	}, ['blocking', 'responseHeaders', 'extraHeaders']);

}


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

// todo: add api to modify the whitelist


function blockRequests() {
	chrome.webRequest.onBeforeRequest.addListener(info => {

		const url = new URL(info.url);

		const matches = config.whitelist.some(wUrl => {
			if (wUrl.startsWith('*')) {
				wUrl = wUrl.slice(1);
				return url.hostname.endsWith(wUrl);
			} else {
				return url.hostname === wUrl;
			}
		});

		return {
			cancel: !matches
		};
	}, {
		urls: [ '<all_urls>' ]
	}, [ 'blocking' ]);
}

function reloadOnError() {
	chrome.webNavigation.onErrorOccurred.addListener(details => {
		// for the moment only reload the main frame and subframe
		// but maybe we need to change this
		if (details.parentFrameId > 0)
			return;

		const tabId = details.tabId;

		setTimeout(() => {
			chrome.tabs.reload(tabId, { bypassCache: true });
		}, 1000);

	});
}

/* Zoom

// getting zoom
const zoom = await chrome.tabs.getZoom();

// setting zoom
await chrome.tabs.setZoom(null, 1);

// listening on zoom change
chrome.tabs.onZoomChange.addListener(({ newZoomFactor, oldZoomFactor, tabId, zoomSettings }));

*/

// function handleConnection() {
// 	let port = null;
// 	const con = new Connection;

// 	// this seems nice
// 	con.on(msg => {
// 		console.log('bg: received msg from con: ', msg);
// 		// todo need to check if we should handle the message
// 		// or if we should pass it to the client
// 		if (port) {
// 			port.postMessage(msg);
// 		}
// 	});

// 	chrome.runtime.onConnect.addListener(p => {
// 		port = p;

// 		// port.name
// 		port.onMessage.addListener(msg => {
// 			// port.postMessage
// 			console.log('bg: received msg: ', msg);

// 			// todo need to check if we should handle the message
// 			// or if we should pass it to the client
// 			con.send(msg);
// 		});

// 		port.onDisconnect.addListener(() => {
// 			console.log('port disconnected');
// 		});
// 	});
// }

main();