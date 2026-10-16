#!/usr/bin/env python3
"""Create only the two fresh, loopback-only #11863 research nodes."""
import json,os,pathlib,socket,subprocess,time
os.umask(0o077)
h=pathlib.Path.home();root=h/'lnd-assets-signet-11863'
image='lightninglabs/lightning-terminal@sha256:fd6d6040f78e0a342437312f3a3c154f4b63d79172c6b466e2dfd242b06b4a1c'
assert not root.exists()
for port in (10089,10099,10129,10139,9789,9799,21863,21864):
 with socket.socket() as s:s.bind(('127.0.0.1',port))
root.mkdir(mode=0o700)
user,password=(h/'wavelength-spike/bitcoin-signet/signet/.cookie').read_text().strip().split(':',1)
for node,rpc,tap,peer,web in [('a',10089,10129,9789,21863),('b',10099,10139,9799,21864)]:
 d=root/node;d.mkdir(mode=0o700);(d/'scratch').mkdir(mode=0o700)
 config={
 'lit-dir':'/data/lit','network':'signet','lnd-mode':'integrated','taproot-assets-mode':'integrated',
 'httpslisten':f'127.0.0.1:{web}','disableui':'true','autopilot.disable':'true','loop-mode':'disable','pool-mode':'disable','faraday-mode':'disable',
 'lnd.lnddir':'/data/lnd','lnd.noseedbackup':'true','lnd.rpclisten':f'127.0.0.1:{rpc}','lnd.norest':'true','lnd.listen':f'127.0.0.1:{peer}','lnd.nobootstrap':'true','lnd.tlsextraip':'127.0.0.1',
 'lnd.bitcoin.node':'bitcoind','lnd.bitcoind.rpchost':'127.0.0.1:38332','lnd.bitcoind.rpcuser':user,'lnd.bitcoind.rpcpass':password,
 'lnd.bitcoind.rpcpolling':'true','lnd.bitcoind.blockpollinginterval':'2s','lnd.bitcoind.txpollinginterval':'2s','lnd.bitcoin.defaultchanconfs':'1',
 'lnd.protocol.simple-taproot-chans':'true','lnd.protocol.simple-taproot-overlay-chans':'true','lnd.protocol.option-scid-alias':'true','lnd.protocol.zero-conf':'true','lnd.rpcmiddleware.enable':'true',
 'lnd.trickledelay':'50','lnd.gossip.sub-batch-delay':'5ms','lnd.caches.rpc-graph-cache-duration':'100ms',
 'taproot-assets.tapddir':'/data/tap','taproot-assets.rpclisten':f'127.0.0.1:{tap}','taproot-assets.restlisten':f'127.0.0.1:{tap+1000}',
 'taproot-assets.universe.public-access':'rw','taproot-assets.universe.sync-all-assets':'true','taproot-assets.allow-public-uni-proof-courier':'true',
 'taproot-assets.proofcourieraddr':'universerpc://127.0.0.1:10089',
 'taproot-assets.experimental.rfq.priceoracleaddress':'use_mock_price_oracle_service_promise_to_not_use_on_mainnet',
 'taproot-assets.experimental.rfq.mockoracleassetsperbtc':'100000000',
 }
 with (d/'lit.conf').open('x') as f:
  f.write('\n'.join(k+'='+v for k,v in config.items())+'\n');f.flush();os.fsync(f.fileno())
 args=['docker','run','-d','--name','oa-lnd-assets-signet-11863-'+node,'--label','issue=11863','--label','purpose=synthetic-signet-assets','--network','host','--user',f'{os.getuid()}:{os.getgid()}','--env','HOME=/data','--env','TMPDIR=/data/scratch','--env','SQLITE_TMPDIR=/data/scratch','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--memory','1g','--cpus','1','--mount',f'type=bind,src={d},dst=/data','--entrypoint','litd',image,'--configfile=/data/lit.conf']
 subprocess.run(args,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=45)
with (root/'setup.json').open('x') as f:json.dump({'network':'signet','image':image,'at_unix':time.time(),'synthetic_only':True,'nodes':['a','b'],'rpc_loopback_only':True,'maximum_initial_funding_sat':5000,'maximum_total_recycled_funding_sat':150000},f,indent=2)
print(json.dumps({'created':True,'network':'signet','nodes':2,'funded':False}))
