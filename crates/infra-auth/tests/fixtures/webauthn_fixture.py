#!/usr/bin/env python3
"""Local signed WebAuthn fixture; never contacts a service or uses a live key."""
import base64
import hashlib
import json
import sys
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec

KEY = serialization.load_pem_private_key(b"""-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgazxOrI31fungcha4
1ejF9zr5JfrVsWo8PO6v+tds2KahRANCAAQ3OJJiw8nna3OogQwIItIqWM34ATgY
udPTumSLU6HtmTzf+snkSDO0yt7+OpdBtIuWMQSucVSZp664VL54HuQa
-----END PRIVATE KEY-----
""", password=None)
CREDENTIAL = b"tankhor-local-test-credential-0001"
USER_HANDLE = bytes.fromhex("019a53a0000070008000000000000001")


def b64(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def uint(value):
    if value < 24:
        return bytes([value])
    if value < 256:
        return b"\x18" + bytes([value])
    raise ValueError("fixture integer too large")


def byte_string(value):
    return bytes([0x40 + len(value)]) + value if len(value) < 24 else b"\x58" + bytes([len(value)]) + value


def text(value):
    encoded = value.encode()
    return bytes([0x60 + len(encoded)]) + encoded


def map_header(size):
    return bytes([0xA0 + size])


def cose_key():
    numbers = KEY.public_key().public_numbers()
    x = numbers.x.to_bytes(32, "big")
    y = numbers.y.to_bytes(32, "big")
    return (map_header(5) + uint(1) + uint(2) + uint(3) + b"\x26" +
            b"\x20" + uint(1) + b"\x21" + byte_string(x) + b"\x22" + byte_string(y))


def attestation(auth_data):
    return (map_header(3) + text("fmt") + text("none") + text("attStmt") +
            map_header(0) + text("authData") + byte_string(auth_data))


def main():
    operation, options_json, origin, counter = sys.argv[1:5]
    options = json.loads(options_json)["publicKey"]
    client_data = json.dumps({
        "type": "webauthn.create" if operation == "register" else "webauthn.get",
        "challenge": options["challenge"],
        "origin": origin,
        "crossOrigin": False,
    }, separators=(",", ":")).encode()
    rp_hash = hashlib.sha256(options["rp"]["id"].encode() if operation == "register" else options["rpId"].encode()).digest()
    if operation == "register":
        auth_data = (rp_hash + b"\x5d" + (0).to_bytes(4, "big") + bytes(16) +
                     len(CREDENTIAL).to_bytes(2, "big") + CREDENTIAL + cose_key())
        response = {
            "id": b64(CREDENTIAL), "rawId": b64(CREDENTIAL), "type": "public-key",
            "response": {"attestationObject": b64(attestation(auth_data)),
                         "clientDataJSON": b64(client_data)},
        }
    else:
        auth_data = rp_hash + b"\x1d" + int(counter).to_bytes(4, "big")
        signature = KEY.sign(auth_data + hashlib.sha256(client_data).digest(), ec.ECDSA(hashes.SHA256()))
        response = {
            "id": b64(CREDENTIAL), "rawId": b64(CREDENTIAL), "type": "public-key",
            "response": {"authenticatorData": b64(auth_data), "clientDataJSON": b64(client_data),
                         "signature": b64(signature), "userHandle": b64(USER_HANDLE)},
        }
    print(json.dumps(response, separators=(",", ":")))


if __name__ == "__main__":
    main()
