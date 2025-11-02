#!/usr/bin/env node

/**
 * Placeholder sync utility that will eventually copy the OpenAPI-generated
 * TypeScript declarations produced by the backend into the frontend
 * `types/generated` directory.
 */

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const thisDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(thisDir, '..');
const targetDir = resolve(projectRoot, 'types/generated');
const placeholderFile = resolve(targetDir, 'index.ts');

await mkdir(targetDir, { recursive: true });

const banner = `// Placeholder for backend OpenAPI-generated types.
// Replace this file with generated outputs once the backend export lands.
export {};
`;

try {
  await writeFile(placeholderFile, banner, { flag: 'wx' });
  console.log(`Created placeholder types at ${placeholderFile}`);
} catch (error) {
  if (error && error.code === 'EEXIST') {
    console.log(`Placeholder types already exist at ${placeholderFile}`);
  } else {
    throw error;
  }
}
