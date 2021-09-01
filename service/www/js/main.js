
import doc, { c } from '/fire-html/doc.js';

function main() {

	const h1 = c('h1', { text: 'Welcome' });

	doc.body.insert(h1);

}

main();