import { HDKey } from '@scure/bip32';
import { sha256 } from '@noble/hashes/sha2.js';
import { keccak_256 } from '@noble/hashes/sha3.js';
import { getPublicKey } from '@noble/secp256k1';

const SEED_HEX = '5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4';
const seed = Buffer.from(SEED_HEX, 'hex');
const master = HDKey.fromMasterSeed(seed);

function toHex(buf) { return Buffer.from(buf).toString('hex'); }

const CHAIN_ID = 42161;

const gasSigner = master.derive(`m/44/${CHAIN_ID}/1/0`);
const preimageKey0 = master.derive(`m/44/${CHAIN_ID}/0/0/0`);
const preimageKey1 = master.derive(`m/44/${CHAIN_ID}/0/0/1`);

function ethAddress(privKey) {
  // getPublicKey with false = uncompressed (65 bytes: 04 || x || y)
  const uncompressed = getPublicKey(privKey, false);
  const hash = keccak_256(uncompressed.slice(1));
  return Buffer.from(hash.slice(12));
}

console.log('=== Gas Signer (m/44/42161/1/0) ===');
console.log(`privkey=${toHex(gasSigner.privateKey)}`);
console.log(`pubkey=${toHex(gasSigner.publicKey)}`);
console.log(`address=0x${ethAddress(gasSigner.privateKey).toString('hex')}`);

console.log('\n=== Preimage Key 0 (m/44/42161/0/0/0) ===');
console.log(`privkey=${toHex(preimageKey0.privateKey)}`);
console.log(`pubkey=${toHex(preimageKey0.publicKey)}`);
console.log(`address=0x${ethAddress(preimageKey0.privateKey).toString('hex')}`);
const pre0 = sha256(preimageKey0.privateKey);
const hash0 = sha256(pre0);
console.log(`preimage=${toHex(pre0)}`);
console.log(`preimage_hash=${toHex(hash0)}`);

console.log('\n=== Preimage Key 1 (m/44/42161/0/0/1) ===');
console.log(`privkey=${toHex(preimageKey1.privateKey)}`);
console.log(`pubkey=${toHex(preimageKey1.publicKey)}`);
console.log(`address=0x${ethAddress(preimageKey1.privateKey).toString('hex')}`);
const pre1 = sha256(preimageKey1.privateKey);
const hash1 = sha256(pre1);
console.log(`preimage=${toHex(pre1)}`);
console.log(`preimage_hash=${toHex(hash1)}`);
