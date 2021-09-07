
function main() {
	const con = new Connection;

	window.addEventListener('message', async ev => {
		// only accept messages from the same frame
		if (ev.source !== window)
			return;

		const msg = ev.data;
		if (!('origin' in msg))
			return;

		if (msg.origin !== 'Client')
			return;

		con.send(msg.data);
	});

}




function timeout(ms) {
	return new Promise(resolve => setTimeout(resolve, ms));
}



class Connection {
	constructor() {
		this.port = null;

		this.connect();
	}

	connect() {
		this.port = chrome.runtime.connect();
		this.port.onMessage.addListener(msg => {
			console.log('content: received message from bg:', msg);
			window.postMessage({
				origin: 'Extension',
				data: msg
			});
		});
		this.port.onDisconnect.addListener(async () => {
			this.port = null;
			console.log('port disconnected');
			await timeout(1000);
			this.connect();
		});
	}

	send(msg) {
		if (this.port) {
			this.port.postMessage(msg);
		}
	}
}




// message communication
// request/response
// maybe pull??



// the client has 3 fns
// request
// send
// receive

main();