
import { c } from '/fire-html/doc.js';
import { link } from '/fire-html/els/link.js';
import Data from '/fire-html/data/data.js';

// install page
export default class Install {
	constructor() {
		this.cont = c('div', { id: 'install-page' });
		this.disks = [];
	}

	get raw() {
		return this.cont.insert(
			...this.disks
		).raw;
	}

	async prepare(con) {

		// should get the disks
		let disks = await con.request('Disks', 'disks');
		this.disks = disks.map(d => {
			const disk = new Disk(d);
			const el = new DiskEl(disk);

			el.btn.onPrevDef('click', async e => {
				const r = await con.request('InstallOn', disk.name);
				console.log('disk installed', r);
				alert('disk installed');
			});

			return el;
		});

	}
}

class Disk extends Data {
	constructor(d) {
		super({
			active: 'bool',
			initialized: 'bool',
			name: 'str',
			size: 'int'
		}, d);
	}
}

class DiskEl {

	constructor(disk) {
		this.cont = c('div', { cls: 'disk' });
		this.name = c('h3', { text: disk.name });
		this.state = c('p', { cls: 'state',
			text: `active: ${ disk.active }, initialized: ${ disk.initialized }`
		});
		this.size = c('p', { cls: 'size',
			text: `${ Math.round(disk.size / 1000000) / 1000 }Gb`
		});

		this.btn = link('#', { text: 'Install immediately' });
	}

	get raw() {
		return this.cont.insert(
			this.name,
			this.state,
			this.size,
			this.btn
		).raw
	}

}