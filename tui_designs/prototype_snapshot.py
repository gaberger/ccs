import os, json, re, sys
L = os.path.expanduser('~/.claude-profiles')
lib = os.path.join(L, 'library')
def body(path, n=40):
    try: t = open(path, encoding='utf-8', errors='replace').read(20000)
    except Exception: return []
    t = re.sub(r'^---\s*\n.*?\n---\s*\n', '', t, count=1, flags=re.S)
    out = []
    for ln in t.splitlines():
        ln = ln.rstrip()
        if not ln and (not out or not out[-1]): continue
        out.append(ln[:160])
        if len(out) >= n: break
    return out
def desc(path):
    try: t = open(path, encoding='utf-8', errors='replace').read(6000)
    except Exception: return ''
    m = re.match(r'---\s*\n(.*?)\n---', t, re.S)
    if not m: return ''
    fm = m.group(1)
    d = re.search(r'^description:\s*(.*(?:\n[ \t]+.*)*)', fm, re.M)
    if not d: return ''
    s = d.group(1).strip()
    if s in ('>', '|', '>-', '|-'): s = ''
    s = re.sub(r'^[>|]-?\s*', '', s)
    s = re.sub(r'\s+', ' ', s).strip().strip('"\'')
    s = s.replace('\\n', ' ')
    return s[:600]
items = []
for root, dirs, files in os.walk(os.path.join(lib, 'skills')):
    dirs.sort()
    if 'SKILL.md' in files:
        rel = os.path.relpath(root, os.path.join(lib, 'skills'))
        folder, name = os.path.split(rel)
        items.append({'k': 's', 'f': folder, 'n': name, 'd': desc(os.path.join(root, 'SKILL.md')), 'b': body(os.path.join(root, 'SKILL.md'))})
        dirs[:] = []
for root, dirs, files in os.walk(os.path.join(lib, 'agents')):
    dirs.sort()
    for fn in sorted(files):
        if fn.endswith('.md'):
            rel = os.path.relpath(os.path.join(root, fn), os.path.join(lib, 'agents'))
            folder, name = os.path.split(rel[:-3])
            items.append({'k': 'a', 'f': folder, 'n': name, 'd': desc(os.path.join(root, fn)), 'b': body(os.path.join(root, fn))})
plugins = []
for p in sorted(os.listdir(os.path.join(lib, 'plugins'))):
    pd = os.path.join(lib, 'plugins', p)
    if not os.path.isdir(pd): continue
    pj = os.path.join(pd, '.claude-plugin', 'plugin.json')
    d = ''
    try: d = json.load(open(pj)).get('description', '')[:140]
    except Exception: pass
    sk = sorted(x for x in os.listdir(os.path.join(pd, 'skills')) if os.path.isdir(os.path.join(pd, 'skills', x))) if os.path.isdir(os.path.join(pd, 'skills')) else []
    ag = sorted(x[:-3] for x in os.listdir(os.path.join(pd, 'agents')) if x.endswith('.md')) if os.path.isdir(os.path.join(pd, 'agents')) else []
    plugins.append({'n': p, 'd': d, 'skills': sk, 'agents': ag})
kits = {}
for k in sorted(os.listdir(os.path.join(L, 'kits'))):
    kd = os.path.join(L, 'kits', k)
    if not os.path.isdir(kd): continue
    u = os.path.join(kd, 'uses')
    lines = [x.strip() for x in open(u).read().splitlines() if x.strip() and not x.startswith('#')] if os.path.exists(u) else []
    kits[k] = {'uses': lines, 'md': os.path.exists(os.path.join(kd, 'CLAUDE.md'))}
profiles = {}
for p in sorted(os.listdir(L)):
    if os.path.exists(os.path.join(L, p, 'profile.env')):
        kf = os.path.join(L, p, 'kits')
        profiles[p] = [x.strip() for x in open(kf).read().splitlines() if x.strip()] if os.path.exists(kf) else []
json.dump({'items': items, 'plugins': plugins, 'kits': kits, 'profiles': profiles}, open('data.json', 'w'), separators=(',', ':'))
print(len(items), sum(1 for i in items if i['k']=='s'), sum(1 for i in items if i['k']=='a'), [ (p['n'], len(p['skills']), len(p['agents'])) for p in plugins], len(kits), profiles)
print(sorted(set((i['k'], i['f']) for i in items)))
print(os.path.getsize('data.json'))
