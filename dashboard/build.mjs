import { build } from 'esbuild';
await build({
  entryPoints: ['src/editor.tsx'], bundle: true, minify: true,
  format: 'esm', target: 'es2022', outdir: 'dist',
  define: { 'process.env.NODE_ENV': '"production"' },
});
