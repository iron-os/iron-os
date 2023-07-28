
import { c } from './util.js';

// install page
export default class Landing {
	constructor() {
		this.el = c('div');
		this.el.classList.add('abs-full');
		this.iframe = c('iframe');
		this.iframe.classList.add('abs-full');

		this.loading = c('p');
		this.loading.id = 'loading-text';
		this.loading.innerText = 'Service: Loading Page';

		this.el.appendChild(this.loading);

		this.active = null;
	}

	get raw() {
		return this.cont.insert(
			this.loading
		).raw;
	}

	openPage(url) {
		if (!url)
			return;

		if (this.active === null) {
			// no page was ever opened
			this.loading.remove();
			this.el.appendChild(this.iframe);
		}
		this.iframe.src = url;
		this.active = url;
	}
}