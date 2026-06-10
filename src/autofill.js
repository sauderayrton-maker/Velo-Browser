// Velo password capture & autofill — injected into every frame at
// document-start. Talks to the Rust side via the "veloPasswords" script
// message handler (login captures) and `window.__veloFill` (autofill).
(() => {
  if (window.__veloAutofillInstalled) return;
  window.__veloAutofillInstalled = true;

  function visible(el) {
    return !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length);
  }

  function findFields() {
    const passwordField = Array.from(document.querySelectorAll('input[type="password"]'))
      .find((el) => visible(el) && !el.disabled);
    if (!passwordField) return null;

    const scope = passwordField.form || document;
    const userField = Array.from(scope.querySelectorAll('input'))
      .filter((el) => el !== passwordField && visible(el) && !el.disabled)
      .find((el) => {
        const type = (el.type || 'text').toLowerCase();
        const autocomplete = (el.autocomplete || '').toLowerCase();
        return autocomplete.includes('username') || autocomplete.includes('email')
          || type === 'email' || type === 'text' || type === 'tel';
      });

    return { passwordField, userField: userField || null };
  }

  function setNativeValue(el, value) {
    const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
    if (setter && setter.set) {
      setter.set.call(el, value);
    } else {
      el.value = value;
    }
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }

  window.__veloFill = (username, password) => {
    const fields = findFields();
    if (!fields) return false;
    if (fields.userField && username) setNativeValue(fields.userField, username);
    if (fields.passwordField && password) setNativeValue(fields.passwordField, password);
    return true;
  };

  function capture() {
    const fields = findFields();
    if (!fields || !fields.passwordField.value) return;
    window.webkit.messageHandlers.veloPasswords.postMessage(JSON.stringify({
      username: fields.userField ? fields.userField.value : '',
      password: fields.passwordField.value,
    }));
  }

  document.addEventListener('submit', capture, true);
  document.addEventListener('click', (event) => {
    const target = event.target.closest('button, input[type="submit"], input[type="button"]');
    if (target) setTimeout(capture, 0);
  }, true);
})();
