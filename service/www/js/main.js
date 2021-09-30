
import doc, { c } from '/fire-html/doc.js';
import { timeout, randomToken } from '/fire-html/util.js';
import Connection from './connection.js';
import Data from '/fire-html/data/data.js';
import Landing from './landing.js';

async function main() {

	const con = new Connection;
	await con.connect();// wait until a connection is made

	const main = c('main');
	doc.body.insert(main);

	// const rawVersionInfo = await con.request('VersionInfo', 'versioninfo');
	// const versionInfo = new VersionInfo(rawVersionInfo);

	// installation should be handled by the user process
	// we also don't provide any api since that can be archieved via
	// the service api

	// probably the only thing we wan't to do here is
	// show debug stuff when in debug

	const page = new Landing;

	await page.prepare(con);

	main.clear();
	main.insert(page);

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