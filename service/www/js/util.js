
export async function timeout(ms) {
	return new Promise(res => {
		setTimeout(res, ms);
	});
}

export function c(el) {
	return document.createElement(el);
}

export const ALPHABET = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';

export function randomToken(length = 8) {
	let s = '';
	for (let i = 0; i < length; i++)
		s += ALPHABET[Math.floor(Math.random() * ALPHABET.length)];
	return s;
}