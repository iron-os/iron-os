import { DefaultTheme, defineConfig } from 'vitepress';

// https://vitepress.dev/reference/site-config
export default defineConfig({
	title: 'Iron OS',
	description: 'Operation system based on buildroot',
	themeConfig: {
		// https://vitepress.dev/reference/default-theme-config
		nav: [
			{ text: 'Home', link: '/' },
			{ text: 'Docs', link: '/docs/getting-started/introduction' },
		],

		sidebar: {
			'/docs/': { base: '/docs/', items: sidebarDocs() },
		},

		socialLinks: [
			{ icon: 'github', link: 'https://github.com/iron-os/iron-os' },
		],
	},
});

function sidebarDocs(): DefaultTheme.SidebarItem[] {
	return [
		{
			text: 'Getting started',
			// collapsed: false,
			items: [
				{
					text: 'Introduction',
					link: 'getting-started/introduction',
				},
				{
					text: 'Requirements',
					link: 'getting-started/requirements',
				},
			],
		},
		{
			text: 'Setup',
			items: [
				{
					text: 'Packages',
					link: 'setup/packages',
				},
				{
					text: 'First Build',
					link: 'setup/first-build',
				},
			],
		},
	];
}
