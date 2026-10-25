#!/usr/bin/env python3
"""End-to-end subprocess benchmark. No shell, downloads or compilation."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import signal
import statistics
import subprocess
import time
import zipfile


def percentile(values, fraction):
    if not values or not 0 < fraction <= 1:
        raise ValueError('nonempty values and percentile in (0,1] required')
    return sorted(values)[math.ceil(len(values) * fraction) - 1]


def host():
    def command(argv):
        try:
            return subprocess.check_output(argv, text=True, stderr=subprocess.STDOUT).strip()
        except (OSError, subprocess.CalledProcessError):
            return None
    return {'captured_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'platform': platform.platform(), 'python': platform.python_version(),
            'cpu': command(['lscpu']), 'ram': command(['free', '-b']),
            'filesystem': command(['df', '-T', str(Path.cwd())]),
            'rustc': command(['rustc', '--version']), 'cargo': command(['cargo', '--version']),
            'power_mode': 'unknown; Windows host power mode must be recorded separately',
            'cwd': str(Path.cwd()), 'logical_cpus': os.cpu_count()}


def profile(path):
    path = Path(path)
    result = {'path': str(path.resolve()), 'bytes': path.stat().st_size,
              'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as z:
            result.update(parts=len(z.infolist()), uncompressed_bytes=sum(i.file_size for i in z.infolist()),
                          media_bytes=sum(i.file_size for i in z.infolist() if '/media/' in i.filename),
                          slides=sum('/slides/' in i.filename and '/_rels/' not in i.filename and i.filename.endswith('.xml') for i in z.infolist()))
    return result


def measure(argv, directory, timeout):
    directory.mkdir(parents=True, exist_ok=True)
    artifact = directory / 'artifact.pptx'
    argv = [str(artifact) if a == '{output}' else a for a in argv]
    rss = None
    timed_out = False
    with (directory/'stdout.bin').open('wb') as stdout, (directory/'stderr.bin').open('wb') as stderr:
        start = time.perf_counter_ns()
        process = subprocess.Popen(argv, stdout=stdout, stderr=stderr, shell=False, start_new_session=os.name == 'posix')
        if hasattr(os, 'wait4'):
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    # Linux reports KiB; macOS reports bytes. This is this child,
                    # not cumulative RUSAGE_CHILDREN or the Python harness.
                    rss = int(usage.ru_maxrss * (1 if platform.system() == 'Darwin' else 1024))
                    break
                if (time.perf_counter_ns() - start) / 1e9 > timeout:
                    timed_out = True
                    os.killpg(process.pid, signal.SIGKILL)
                time.sleep(.001)
        else:
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                process.wait()
        wall = (time.perf_counter_ns() - start) / 1e6
    return {'argv': argv, 'wall_ms': wall, 'returncode': process.returncode,
            'timed_out': timed_out, 'peak_rss_bytes': rss,
            'rss_source': 'wait4 child ru_maxrss (includes waited descendants on Linux)' if rss is not None else 'unknown; wait4 unavailable',
            'stdout_bytes': (directory/'stdout.bin').stat().st_size,
            'stderr_bytes': (directory/'stderr.bin').stat().st_size,
            'artifact_bytes': artifact.stat().st_size if artifact.exists() else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--result-dir', type=Path, required=True, help='new directory; never overwrites a run')
    parser.add_argument('--warmups', type=int, default=5)
    parser.add_argument('--samples', type=int, default=50)
    parser.add_argument('--timeout', type=float, default=60)
    parser.add_argument('--cache-state', required=True, help='e.g. warm-uncontrolled; restart is not cold FS')
    parser.add_argument('--filesystem-label', required=True, help='e.g. WSL-ext4, Windows-NTFS, UNC')
    parser.add_argument('--input', type=Path)
    parser.add_argument('--host-only', action='store_true')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.warmups < 0 or args.samples < 1 or args.timeout <= 0:
        parser.error('warmups >= 0, samples >= 1 and timeout > 0 required')
    argv = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not argv and not args.host_only:
        parser.error('command argv required after --')
    args.result_dir.mkdir(parents=True, exist_ok=False)
    report = {'schema_version': 1, 'host': host(), 'cache_state': args.cache_state,
              'filesystem_label': args.filesystem_label, 'warmups': args.warmups,
              'sample_count': args.samples, 'command': argv, 'advisory': True,
              'stage_breakdown': None, 'stage_breakdown_reason': 'CLI must expose internal stages; wall time is complete subprocess lifetime',
              'input': profile(args.input) if args.input else None, 'runs': []}
    if not args.host_only:
        for i in range(args.warmups + args.samples):
            row = measure(argv, args.result_dir / f'run-{i:03}', args.timeout)
            row['warmup'] = i < args.warmups
            report['runs'].append(row)
            if row['returncode'] != 0:
                report['failure'] = f'run {i} failed; no latency summary'
                break
        else:
            samples = report['runs'][args.warmups:]
            times = [r['wall_ms'] for r in samples]
            memory = [r['peak_rss_bytes'] for r in samples if r['peak_rss_bytes'] is not None]
            report['summary'] = {'median_ms': statistics.median(times), 'p95_ms': percentile(times, .95),
                                 'peak_rss_bytes': max(memory) if memory else None,
                                 'artifact_bytes': [r['artifact_bytes'] for r in samples],
                                 'stdout_bytes': [r['stdout_bytes'] for r in samples]}
    (args.result_dir/'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'manifest': str(args.result_dir/'manifest.json'), 'summary': report.get('summary'), 'failure': report.get('failure')}))
    return 1 if 'failure' in report else 0

if __name__ == '__main__': raise SystemExit(main())
