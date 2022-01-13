
import { timeout, c } from './util.js';
import Connection from './connection.js';
import Landing from './landing.js';

async function main() {

	const con = new Connection;
	await con.connect();// wait until a connection is made

	const main = c('main');
	main.classList.add('abs-full');
	document.body.appendChild(main);

	// const rawVersionInfo = await con.request('VersionInfo', 'versioninfo');
	// const versionInfo = new VersionInfo(rawVersionInfo);

	// installation should be handled by the user process
	// we also don't provide any api since that can be archieved via
	// the service api

	// probably the only thing we wan't to do here is
	// show debug stuff when in debug

	const page = new Landing;

	await page.prepare(con);

	// main.clear();
	main.appendChild(page.el);

}

// class VersionInfo extends Data {
// 	constructor(d) {
// 		super({
// 			version_str: 'str',
// 			version: 'str',
// 			signature: 'optstr',
// 			installed: 'bool'
// 		}, d);
// 	}
// }


main();