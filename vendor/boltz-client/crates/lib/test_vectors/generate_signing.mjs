/**
 * Generate signing test vectors using ethers v6 — the same library
 * the Boltz web app uses for Alchemy signing.
 *
 * Produces expected signatures for:
 * 1. Raw ECDSA (EIP-7702 authorization) — signingKey.sign(digest).serialized
 * 2. EIP-191 personal_sign (UserOp) — wallet.signMessage(getBytes(payload))
 *
 * Run: npm install && node generate_signing.mjs
 */
import { HDKey } from '@scure/bip32';
import { Wallet, getBytes } from 'ethers';

const SEED_HEX = '5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4';
const CHAIN_ID = 42161;

// Derive gas signer private key using the same BIP-32 path as the Rust code
const seed = Buffer.from(SEED_HEX, 'hex');
const master = HDKey.fromMasterSeed(seed);
const gasSigner = master.derive(`m/44/${CHAIN_ID}/1/0`);
const privateKeyHex = '0x' + Buffer.from(gasSigner.privateKey).toString('hex');

// Create ethers Wallet — same as the Boltz web app uses
const wallet = new Wallet(privateKeyHex);

console.log(`=== Signer Info ===`);
console.log(`privateKey=${privateKeyHex}`);
console.log(`address=${wallet.address}`);
console.log();

// ─── Test Vector 1: Raw ECDSA sign (EIP-7702 authorization) ──────────
// This is what the web app does: signer.signingKey.sign(rawPayload).serialized
const authPayload = '0x' + '01'.repeat(32);
const authSig = wallet.signingKey.sign(authPayload).serialized;

console.log(`=== Vector 1: Raw ECDSA (authorization) ===`);
console.log(`payload=${authPayload}`);
console.log(`signature=${authSig}`);
console.log();

// ─── Test Vector 2: EIP-191 signMessage (UserOp) ─────────────────────
// This is what the web app does: await signer.signMessage(getBytes(rawPayload))
const uoPayload = '0x' + '02'.repeat(32);
const uoSig = await wallet.signMessage(getBytes(uoPayload));

console.log(`=== Vector 2: EIP-191 signMessage (UserOp) ===`);
console.log(`payload=${uoPayload}`);
console.log(`signature=${uoSig}`);
console.log();

// ─── Test Vector 3: Full signPreparedCalls flow (first-time) ─────────
// Simulate the exact signPreparedCalls logic from the web app
const mockFirstTimeResponse = {
    type: "array",
    data: [
        {
            type: "authorization",
            data: { address: "0x69007702764179f14F51cdce752f4f775d74E139", nonce: "0x0" },
            signatureRequest: { rawPayload: '0x' + 'aa'.repeat(32), type: "eip7702Auth" },
            chainId: "0xa4b1",
        },
        {
            type: "user-operation-v070",
            data: { sender: wallet.address },
            signatureRequest: { data: { raw: '0x' + 'bb'.repeat(32) } },
            chainId: "0xa4b1",
        },
    ],
};

// Replicate signPreparedCalls from Alchemy.ts lines 218-277
const entries = mockFirstTimeResponse.data;
const authEntry = entries[0];
const uoEntry = entries[1];

const authSignature = wallet.signingKey.sign(authEntry.signatureRequest.rawPayload).serialized;
const uoSignature = await wallet.signMessage(getBytes(uoEntry.signatureRequest.data.raw));

const signedFirstTime = [
    {
        type: authEntry.type,
        data: authEntry.data,
        chainId: authEntry.chainId,
        signature: { type: "secp256k1", data: authSignature },
    },
    {
        type: uoEntry.type,
        data: uoEntry.data,
        chainId: uoEntry.chainId,
        signature: { type: "secp256k1", data: uoSignature },
    },
];

console.log(`=== Vector 3: First-time signPreparedCalls ===`);
console.log(`auth_payload=${authEntry.signatureRequest.rawPayload}`);
console.log(`auth_signature=${authSignature}`);
console.log(`uo_payload=${uoEntry.signatureRequest.data.raw}`);
console.log(`uo_signature=${uoSignature}`);
console.log();
console.log(`signed_output=${JSON.stringify({ type: "array", data: signedFirstTime }, null, 2)}`);
console.log();

// ─── Test Vector 4: Full signPreparedCalls flow (subsequent) ─────────
const mockSubsequentResponse = {
    type: "user-operation-v070",
    data: { sender: wallet.address },
    signatureRequest: { data: { raw: '0x' + 'cc'.repeat(32) } },
    chainId: "0xa4b1",
};

const subPayload = mockSubsequentResponse.signatureRequest.data.raw;
const subSignature = await wallet.signMessage(getBytes(subPayload));

const signedSubsequent = {
    type: mockSubsequentResponse.type,
    data: mockSubsequentResponse.data,
    chainId: mockSubsequentResponse.chainId,
    signature: { type: "secp256k1", data: subSignature },
};

console.log(`=== Vector 4: Subsequent signPreparedCalls ===`);
console.log(`payload=${subPayload}`);
console.log(`signature=${subSignature}`);
console.log();
console.log(`signed_output=${JSON.stringify(signedSubsequent, null, 2)}`);
