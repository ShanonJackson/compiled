// Generates CSS test fixtures of varying sizes for benchmarking.
// Each fixture exercises different CSS features to stress different parser paths.
const fs = require('fs');
const path = require('path');

const dir = path.join(__dirname, 'fixtures');
fs.mkdirSync(dir, { recursive: true });

// ── Small: typical component CSS (~50 rules) ──────────────────────────────
function generateSmall() {
  const lines = [];
  const props = ['color', 'background', 'font-size', 'margin', 'padding',
    'border', 'display', 'position', 'width', 'height', 'line-height',
    'text-align', 'overflow', 'z-index', 'opacity'];
  const values = ['red', '#fff', '12px', '0', '1em', 'none', 'block',
    'relative', '100%', 'auto', '1.5', 'center', 'hidden', '10', '0.5'];

  for (let i = 0; i < 50; i++) {
    lines.push(`.component-${i} {`);
    const numDecls = 3 + (i % 5);
    for (let j = 0; j < numDecls; j++) {
      const p = props[(i * 3 + j) % props.length];
      const v = values[(i * 7 + j) % values.length];
      lines.push(`    ${p}: ${v};`);
    }
    lines.push('}');
    lines.push('');
  }
  return lines.join('\n');
}

// ── Medium: real-world-ish stylesheet (~500 rules with at-rules) ──────────
function generateMedium() {
  const lines = [];
  const props = ['color', 'background-color', 'font-size', 'margin-top',
    'margin-bottom', 'padding-left', 'padding-right', 'border-radius',
    'box-shadow', 'transform', 'transition', 'display', 'flex-direction',
    'justify-content', 'align-items', 'gap', 'grid-template-columns',
    'font-weight', 'text-decoration', 'cursor'];

  // Global styles
  lines.push('/* Reset styles */');
  lines.push('*, *::before, *::after {');
  lines.push('    box-sizing: border-box;');
  lines.push('    margin: 0;');
  lines.push('    padding: 0;');
  lines.push('}');
  lines.push('');

  // Component styles
  for (let i = 0; i < 200; i++) {
    if (i % 40 === 0) {
      lines.push(`/* Section ${i / 40 + 1} */`);
    }
    const sel = i % 3 === 0 ? `.btn-${i}` :
                i % 3 === 1 ? `.card-${i} .card-body` :
                              `#app .layout .section-${i}`;
    lines.push(`${sel} {`);
    const numDecls = 3 + (i % 8);
    for (let j = 0; j < numDecls; j++) {
      const p = props[(i + j) % props.length];
      lines.push(`    ${p}: ${generateValue(p, i, j)};`);
    }
    lines.push('}');
    lines.push('');
  }

  // Media queries
  const breakpoints = ['480px', '768px', '1024px', '1200px'];
  for (const bp of breakpoints) {
    lines.push(`@media (min-width: ${bp}) {`);
    for (let i = 0; i < 30; i++) {
      lines.push(`    .responsive-${i} {`);
      lines.push(`        font-size: ${12 + i}px;`);
      lines.push(`        padding: ${i}px;`);
      lines.push('    }');
      lines.push('');
    }
    lines.push('}');
    lines.push('');
  }

  // Keyframes
  for (let i = 0; i < 10; i++) {
    lines.push(`@keyframes animation-${i} {`);
    lines.push('    0% {');
    lines.push(`        opacity: 0;`);
    lines.push(`        transform: translateY(${i * 10}px);`);
    lines.push('    }');
    lines.push('    100% {');
    lines.push(`        opacity: 1;`);
    lines.push(`        transform: translateY(0);`);
    lines.push('    }');
    lines.push('}');
    lines.push('');
  }

  return lines.join('\n');
}

// ── Large: stress test (~2000 rules) ──────────────────────────────────────
function generateLarge() {
  const chunks = [];
  for (let i = 0; i < 10; i++) {
    chunks.push(generateMedium().replace(/\d+/g, (m) => String(Number(m) + i * 10000)));
  }
  return chunks.join('\n\n');
}

function generateValue(prop, i, j) {
  if (prop.includes('color')) return `#${((i * 123 + j * 456) % 0xFFFFFF).toString(16).padStart(6, '0')}`;
  if (prop.includes('size') || prop.includes('margin') || prop.includes('padding') || prop.includes('gap'))
    return `${(i + j) % 48}px`;
  if (prop.includes('radius')) return `${(i % 20)}px`;
  if (prop.includes('shadow')) return `0 ${i % 4}px ${i % 8}px rgba(0, 0, 0, 0.${i % 5})`;
  if (prop.includes('transform')) return `translateX(${i}px)`;
  if (prop.includes('transition')) return `all 0.${i % 5}s ease`;
  if (prop.includes('display')) return ['flex', 'grid', 'block', 'inline-flex'][i % 4];
  if (prop.includes('direction')) return ['row', 'column', 'row-reverse'][i % 3];
  if (prop.includes('justify') || prop.includes('align')) return ['center', 'flex-start', 'flex-end', 'space-between'][i % 4];
  if (prop.includes('grid')) return `repeat(${(i % 4) + 1}, 1fr)`;
  if (prop.includes('weight')) return [400, 500, 600, 700][i % 4];
  if (prop.includes('decoration')) return ['none', 'underline'][i % 2];
  if (prop.includes('cursor')) return ['pointer', 'default', 'grab'][i % 3];
  return `${i + j}px`;
}

// ── Write fixtures ────────────────────────────────────────────────────────
const small = generateSmall();
const medium = generateMedium();
const large = generateLarge();

fs.writeFileSync(path.join(dir, 'small.css'), small);
fs.writeFileSync(path.join(dir, 'medium.css'), medium);
fs.writeFileSync(path.join(dir, 'large.css'), large);

console.log(`Generated fixtures:`);
console.log(`  small.css:  ${(small.length / 1024).toFixed(1)} KB  (${small.split('\n').length} lines)`);
console.log(`  medium.css: ${(medium.length / 1024).toFixed(1)} KB  (${medium.split('\n').length} lines)`);
console.log(`  large.css:  ${(large.length / 1024).toFixed(1)} KB  (${large.split('\n').length} lines)`);
