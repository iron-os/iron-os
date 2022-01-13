
import { timeout } from './util.js';

export default class Websocket {

	// if you create a new connection
	// it will automatically connect
	constructor() {
		// raw websocket connection
		this.ws = null;

		this.onFn = () => {};
		this.connectAlways();
	}

	async on(fn) {
		this.onFn = fn;
	}

	// throws if no connection is established
	// or the data could not be serialized
	send(data = null) {
		if (data === null)
			throw new Error('data is null');
		if (!this.ws)
			throw new Error('websocket closed');
		data = JSON.stringify(data);
		this.ws.send(data);
	}

	//----- Private

	// does not throw
	async connectAlways() {
		while (true) {

			try {
				await this.connect();
				// we got connected
				if (this.ws !== null)
					return;
			} catch (e) {
				// retry after 1 second
				console.log("could not connect error:", e);
				await timeout(1000);
			}

		}
	}

	// TODO check if error is handled correctly
	async connect() {
		// const prot = window.location.protocol.startsWith('https') ? 'wss' : 'ws';
		this.ws = new WebSocket(`ws://127.0.0.1:8888/websocket`);

		this.ws.addEventListener('close', async e => {
			this.ws = null;
			// connection closed
			// lets retry
			console.log('connection closed');
			await timeout(2000);
			await this.connectAlways();
		});

		this.ws.addEventListener('message', msg => {
			const data = msg.data;
			if (data === null)
				throw new Error('received message null');

			const ser = JSON.parse(data);

			this.onFn(ser);
		});

		return new Promise(resolve => {
			const established = () => {
				// gc
				this.ws.removeEventListener('open', established);
				resolve();
			};
			this.ws.addEventListener('open', established);
		});
	}
}