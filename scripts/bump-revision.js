#!/usr/bin/env node
/*
 * Advance the build revision letter in package.json and print the display
 * version for the build.
 *
 * The semver "version" stays x.y.z; "buildRevision" holds the letter and
 * "buildRevisionOf" the version it belongs to. When the version changes,
 * the letter restarts at "a". After "z" comes "aa".
 *
 * The display version drops a zero patch number (1.0.0 -> 1.0). Release builds
 * show just that; debug builds add the letter (1.0b) so each one has its own
 * output folder and installer name. rust/app/build.rs applies the same rule.
 *
 * Usage: node scripts/bump-revision.js [--print] [--debug]
 *   --print  show the current display version without changing anything
 *   --debug  the display version of a debug build (with the letter)
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

const debug = process.argv.includes('--debug');

function displayVersion(letter) {
  const short = pkg.version.replace(/^(\d+\.\d+)\.0$/, '$1');
  return debug ? `${short}${letter}` : short;
}

if (process.argv.includes('--print')) {
  // A letter left over from an older version means no build of this version yet.
  const letter = pkg.buildRevisionOf === pkg.version ? pkg.buildRevision || '' : '';
  process.stdout.write(`${displayVersion(letter)}\n`);
  process.exit(0);
}

const sameVersion = pkg.buildRevisionOf === pkg.version;
pkg.buildRevision = sameVersion ? nextLetters(String(pkg.buildRevision || '')) : 'a';
pkg.buildRevisionOf = pkg.version;

const indent = raw.match(/^(\s+)"/m)?.[1] || '  ';
fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, indent)}\n`);
process.stdout.write(`${displayVersion(pkg.buildRevision)}\n`);
