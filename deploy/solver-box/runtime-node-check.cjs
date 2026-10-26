// Execute inside the image, not on the build host. Aethyr's controller tests
// need node:test, vm and Web Crypto, without npm or a model/provider login.
const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm');
test('the cloud runtime can execute the browser-controller test toolchain',()=>{
 assert.equal(typeof globalThis.crypto?.getRandomValues,'function');
 const values=vm.runInNewContext('Array.from(crypto.getRandomValues(new Uint32Array(4)))',{crypto:globalThis.crypto});
 assert.equal(values.length,4);assert.ok(values.every(Number.isSafeInteger));
});
