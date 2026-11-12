(() => {
  "use strict";

  const fromBase64url = (value) => {
    const base64url = value.replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64url + "=".repeat((4 - (base64url.length % 4)) % 4);
    const bytes = Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
    return bytes.buffer;
  };

  const toBase64url = (value) => {
    const buffer = value instanceof ArrayBuffer ? value : value.buffer;
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
  };

  const form = document.querySelector('form[action$="/passkey/finish"]');
  const optionsElement = document.getElementById("passkey-options");
  if (!form || !optionsElement) return;

  form.addEventListener("submit", async (event) => {
    if (form.elements.response.value) return;
    event.preventDefault();
    const status = document.getElementById("passkey-status");
    try {
      if (!navigator.credentials) throw new Error("Passkeys are unavailable in this browser.");
      const publicOptions = JSON.parse(optionsElement.textContent);
      publicOptions.challenge = fromBase64url(publicOptions.challenge);
      if (publicOptions.allowCredentials) {
        publicOptions.allowCredentials = publicOptions.allowCredentials.map((descriptor) => ({
          ...descriptor,
          id: fromBase64url(descriptor.id),
        }));
      }
      const credential = await navigator.credentials.get({ publicKey: publicOptions });
      if (!credential) throw new Error("No passkey was selected.");
      const assertion = credential.response;
      const serialized = {
        id: credential.id,
        rawId: toBase64url(credential.rawId),
        type: credential.type,
        response: {
          clientDataJSON: toBase64url(assertion.clientDataJSON),
          authenticatorData: toBase64url(assertion.authenticatorData),
          signature: toBase64url(assertion.signature),
          userHandle: assertion.userHandle ? toBase64url(assertion.userHandle) : null,
        },
      };
      form.elements.response.value = JSON.stringify(serialized);
      form.requestSubmit();
    } catch (error) {
      if (status) status.textContent = error instanceof Error ? error.message : "Passkey verification failed.";
    }
  });
})();
