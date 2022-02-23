
import { timeout, c } from './util.js';
import Connection from './connection.js';
import Landing from './landing.js';

async function main() {

	const main = c('main');
	main.classList.add('abs-full');
	document.body.appendChild(main);

	const page = new Landing;

	const con = new Connection;
	con.onOpenPage(url => {
		page.openPage(url);
	});

	con.connect();// wait until a connection is made

	// main.clear();
	main.appendChild(page.el);

}

main();