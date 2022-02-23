
import { timeout } from './util.js';

export default class Connection {
	constructor() {
		this.openPageFn = () => {};
	}

	onOpenPage(fn) {
		this.openPageFn = fn;
	}

	// does not throw
	connect() {
		const ws = new WebSocket('ws://127.0.0.1:8888/onopenpage');

		ws.addEventListener('close', async e => {
			console.log('connection failed');
			// wait 2s before retrying
			await timeout(2000);
			this.connect();
		});

		ws.addEventListener('message', msg => {
			const data = msg.data;
			if (typeof data === 'string')
				this.openPageFn(data);
			else
				console.log('received unknown data', data);
		});
	}
}