
import doc, { c } from '/fire-html/doc.js';
import { timeout, randomToken } from '/fire-html/util.js';
import Connection from './connection.js';
import Data from '/fire-html/data/data.js';

async function main() {

	const h1 = c('h1', { text: 'Welcome' });
	const div = c('div');

	doc.body.insert(h1);
	doc.body.insert(div);

	// const con = new Connection;

	// con.on(msg => {
	// 	console.log('received a message');
	// });

	// setInterval(() => {
	// 	window.postMessage({ cls: 'nice' });
	// }, 1000);

	await timeout(1000);

	const con = new Connection;
	const r = await con.request('VersionInfo', 'hey');

	const version = new VersionInfo(r);
	console.log('version', version.export());

	div.insert(
		field('version: ', version.version),
		field('buildroot: ', version.buildroot_version),
		field('installed: ', version.installed),
		field('channel: ', version.channel)
	);

}

function field(name, value) {
	const g = c('p');
	const nameEl = c('b', { text: name });
	const valueEl = c('span', { text: value });
	return g.insert(
		nameEl,
		valueEl
	)
}


class VersionInfo extends Data {
	constructor(d) {
		super({
			buildroot_version: 'str',
			version: 'int',
			installed: 'bool',
			channel: 'str'
		}, d);
	}
}


main();