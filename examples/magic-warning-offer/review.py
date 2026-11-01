import hashlib,json,os,subprocess
from pathlib import Path
p=json.loads(Path(os.environ['SIMON_PACKET']).read_text())
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
if p['gate']!='reviewer' or head!=p['candidate_commit']: raise ValueError('candidate identity mismatch')
changed=subprocess.check_output(['git','diff','--name-only',p['baseline_commit'],head],text=True).splitlines()
if any(x!='warning-repair.json' and not x.startswith('manabrew-rs/crates/manabrew-engine/') for x in changed): raise ValueError('candidate exceeds requested source scope')
archive=Path(os.environ['SIMON_MAGIC_ARCHIVE'])
def digest():
    h=hashlib.sha256()
    with archive.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()
expected='9fede9dbee4caed9e3774391215e040c23e142c0cc2e382b1ceeecb12ea2859b'
if digest()!=expected: raise ValueError('actual archive identity changed')
env=dict(os.environ,CARGO_TARGET_DIR=str(Path(os.environ['SIMON_SCRATCH'])/'target'),CARGO_NET_OFFLINE='true',DYLD_SHARED_REGION='avoid')
r=subprocess.run(['cargo','test','--jobs','1','-p','manabrew-engine','--lib','origin_offer_regression','--','--nocapture'],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=1700)
Path(os.environ['SIMON_SCRATCH'],'review-tests.log').write_text(r.stdout)
if digest()!=expected: raise ValueError('archive identity changed during review')
ok=r.returncode==0 and 'running 3 tests' in r.stdout and '3 passed; 0 failed' in r.stdout
print(json.dumps({'action_id':p['action_id'],'outcome':'pass' if ok else 'changes_required','report':'All three frozen archived-card action-space controls passed; runtime/game correctness not certified.' if ok else 'Frozen archived-card action-space controls failed: '+r.stdout[-3000:],'candidate_commit':None,'inspected_commit':head}))
