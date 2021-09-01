
window.addEventListener("message", ev => {
	// only accept messages from the same frame
	if (ev.source !== window)
		return;

	const msg = ev.data;

	if (typeof msg !== 'string')
		return;

	chrome.runtime.sendMessage(msg);
});