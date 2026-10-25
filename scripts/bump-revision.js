#!/usr/bin/env node
/*
 * Advance the build revision letter in package.json (0.8.0a -> 0.8.0b).
 *
 * The semver "version" stays x.y.z; "buildRevision" holds the letter and
 * "buildRevisionOf" the version it belongs to. When the version changes,
 * the letter restarts at "a". After "z" comes "aa".
 *
 * Usage: node scripts/bump-revision.js [--print]
 *   --print  show the current display version without changing anything
 */
const fs = require('node:fs');
const path = require('node:path');

const pkgPath = path.join(__dirname, '..', 'package.json');
const raw = fs.readFileSync(pkgPath, 'utf8');
const pkg = JSON.parse(raw);

function nextLetters(letters) {
  if (!letters) return 'a';
  const chars = letters.split('');
  let i = chars.length - 1;
  while (i >= 0 && chars[i] === 'z') {
    chars[i] = 'a';
    i -= 1;
  }
  if (i < 0) return `a${chars.join('')}`;
  chars[i] = String.fromCharCode(chars[i].charCodeAt(0) + 1);
  return chars.join('');
}

if (process.argv.includes('--print')) {
  process.stdout.write(`${pkg.version}${pkg.buildRevision || ''}\n`);
  process.exit(0);
}

const sameVersion = pkg.buildRevisionOf === pkg.version;
pkg.buildRevision = sameVersion ? nextLetters(String(pkg.buildRevision || '')) : 'a';
pkg.buildRevisionOf = pkg.version;

const indent = raw.match(/^(\s+)"/m)?.[1] || '  ';
fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, indent)}\n`);
process.stdout.write(`${pkg.version}${pkg.buildRevision}\n`);
