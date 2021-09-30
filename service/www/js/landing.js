
import { c } from '/fire-html/doc.js';
import { link } from '/fire-html/els/link.js';
import Data from '/fire-html/data/data.js';

// install page
export default class Landing {
	constructor() {
		this.cont = c('div', { id: 'landing-page', cls: 'loading' });
		this.iframe = c('iframe');
		this.loading = c('p', { text: 'Loading' });
		this.active = null;
	}

	get raw() {
		return this.cont.insert(
			this.loading
		).raw;
	}

	openPage(url) {
		if (this.active === null) {
			// no page was ever opened
			this.cont.cls.remove('loading');
			this.cont.clear();
			this.cont.insert(this.iframe);
		}
		this.iframe.attr.src = url;
		this.active = url;
	}

	async prepare(con) {

		con.requestStream('OpenPageStream', '', url => {
			if (url === '')
				return;
			// todo maybe check if that page really could be opened
			this.openPage(url);
		});

	}
}