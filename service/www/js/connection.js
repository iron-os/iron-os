
import { timeout, randomToken } from '/fire-html/util.js';
import Websocket from './websocket.js';

/*
struct Message {
	id: RandomToken,
	kind: Request|Push|Response,
	name: String, // the name that identifiers this message
				// for example DisksInfo
				// or InstallTo
	data: T
}
*/

export default class Connection {
	constructor() {
		// fn(name, data)
		this.onFn = () => {};

		// contains {id: fn(data)}
		this.requests = {};
		// contains {id: fn(data)}
		this.streams = {};

		this.ws = new Websocket;
	}

	async connect() {
		await this.ws.connect();

		this.ws.on(msg => {
			console.log('client: receive data: ', msg);

			// let's just fail if the give values are not found
			let fn;
			switch (msg.kind) {
				case 'Request':
					throw new Error('Request not allowed');
					break;
				case 'Push':
					fn = this.streams[msg.id];
					if (!fn)
						throw new Error('request with id not found: ' + msg.id);
					fn(msg.data);
					break;
				case 'Response':
					fn = this.requests[msg.id];
					if (!fn)
						throw new Error('request with id not found: ' + msg.id);
					fn(msg.data);
					delete this.requests[msg.id];
					break;
			}
		});
	}

	async request(name, data) {
		return new Promise(resolve => {
			const id = randomToken(12);
			const msg = {
				id,
				kind: 'Request',
				name,
				data
			};

			this.requests[id] = resolve;

			this.ws.send(msg);
		});
	}

	requestStream(name, data, fn) {
		const id = randomToken(12);
		const msg = {
			id,
			kind: 'RequestStream',
			name,
			data
		};

		this.streams[id] = fn;

		this.ws.send(msg);
	}
}