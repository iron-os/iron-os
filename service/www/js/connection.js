import { timeout } from './util.js';

export default class Connection {
	constructor() {
		this.openPageFn = () => {};
		this.interval = null;
	}

	onOpenPage(fn) {
		this.openPageFn = fn;
	}

	// does not throw
	connect() {
		if (this.interval) {
			clearInterval(this.interval);
			this.interval = null;
		}

		const ws = new WebSocket('ws://127.0.0.1:8888/service-stream');

		ws.addEventListener('open', e => {
			try {
				ws.send('StillAlive');
			} catch (e) {
				console.log('failed to do initial requests', e);
			}
		});

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

		this.interval = setInterval(() => {
			try {
				ws.send('StillAlive');
			} catch (e) {
				console.log('could not send watchdog');
			}
		}, 5000);
	}
}