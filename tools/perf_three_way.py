"""Three pinned executables, alternating processes, decoded output and CPU checks.

Run only after all builds/profilers finish. Requires Windows and Pillow.
Usage: python perf_three_way.py BASELINE FIRST CANDIDATE OUTPUT_DIR
The render path excludes window dispatch, input injection and BitBlt.
"""
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import time
from perf_memory import get_times, image_identity

PATTERN = re.compile(r"bench: (\d+) frames, frame median ([\d.]+) ms \(min ([\d.]+), max ([\d.]+)\)")

def run(command, cwd=None, env=None):
    start=time.perf_counter()
    with subprocess.Popen(command,cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE) as process:
        try:
            out,err=process.communicate(timeout=180)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            raise
        wall=(time.perf_counter()-start)*1000
        creation,exit_time,kernel,user=(w.FILETIME() for _ in range(4))
        if not get_times(int(process._handle),c.byref(creation),c.byref(exit_time),c.byref(kernel),c.byref(user)):
            raise c.WinError(c.get_last_error())
        ticks=lambda t: (t.dwHighDateTime<<32)|t.dwLowDateTime
        result=dict(command=command,returncode=process.returncode,wall_ms=wall,
                    cpu_ms=(ticks(kernel)+ticks(user))/10000,
                    stdout=out.decode('utf-8',errors='replace'),stderr=err.decode('utf-8',errors='replace'))
    if process.returncode: raise RuntimeError(result)
    return result

def main():
    baseline,first,candidate,directory=map(Path,sys.argv[1:])
    directory.mkdir(parents=True,exist_ok=True)
    executables=dict(zip(['baseline','first','candidate'],[p.resolve() for p in [baseline,first,candidate]]))
    env=dict(os.environ,OSK_CONFIG=str((directory/'isolated-config.json').resolve()))
    for key in ['OSK_THEME','OSK_LAYOUT']: env.pop(key,None)
    report=dict(platform=platform.platform(),python=sys.version,cpu_count=os.cpu_count(),runs=[],valid=False,
                executables={k:dict(path=str(p),size=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for k,p in executables.items()})
    output=directory/'timing.json'
    def save(): output.write_text(json.dumps(report,indent=2),encoding='utf-8')
    save()
    scenarios={
        'hover_800':['--hover','A'], 'menu_800':['--menu'], 'pressed_800':['--pressed','A'],
        'hover_1440':['--hover','A'], 'menu_1440':['--menu'],
    }
    for scenario,args in scenarios.items():
        size='1440x540' if scenario.endswith('1440') else '800x300'
        expected=None
        for trial in range(5):
            order=['baseline','first','candidate'] if trial%2==0 else ['candidate','first','baseline']
            # Rotate the middle position too, so no variant is always second.
            order=order[trial%3:]+order[:trial%3]
            for side in order:
                image=directory/f'{scenario}-{side}.png'
                image.unlink(missing_ok=True)
                record=dict(side=side,scenario=scenario,trial=trial,valid=False)
                report['runs'].append(record)
                try:
                    record.update(run([str(executables[side]),'--render',str(image.resolve()),'--size',size,'--layout','Latino',*args,'--bench-frames','500'],env=env))
                    match=PATTERN.search(record['stderr'])
                    if not match or int(match[1])!=500: raise ValueError('missing work count')
                    record.update(frames=500,median_ms=float(match[2]),min_ms=float(match[3]),max_ms=float(match[4]))
                    record.update(image_identity(image,size))
                    if expected is None: expected=record['pixels_sha256']
                    if expected!=record['pixels_sha256']: raise ValueError('pixel mismatch')
                    record['valid']=True
                except Exception as error:
                    record['error']=str(error)
                    save()
                    raise
                save()
        summary={side: {metric: {'median':statistics.median(values),'range':[min(values),max(values)]}
                           for metric in ['median_ms','wall_ms','cpu_ms']
                           for values in [[r[metric] for r in report['runs'] if r['scenario']==scenario and r['side']==side]]}
                 for side in executables}
        report.setdefault('summary',{})[scenario]=summary
        print(scenario,json.dumps(summary),flush=True)
    report['valid']=True
    save()

if __name__=='__main__': main()
