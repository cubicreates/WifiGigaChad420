import fs from 'node:fs';
import path from 'node:path';
import { stripTypeScriptTypes } from 'node:module';

const uiDir = process.argv[2];
const outHtmlPath = process.argv[3];

const cssPath = path.join(uiDir, 'src', 'styles', 'app.css');
const cssContent = fs.readFileSync(cssPath, 'utf8');

const tsxFiles = [
  path.join(uiDir, 'src', 'components', 'UpdateModal.tsx'),
  path.join(uiDir, 'src', 'components', 'Sidebar.tsx'),
  path.join(uiDir, 'src', 'screens', 'InstallerUninstallerScreens.tsx'),
  path.join(uiDir, 'src', 'screens', 'WifiDashboardScreen.tsx'),
  path.join(uiDir, 'src', 'screens', 'OngoingProcessesScreen.tsx'),
  path.join(uiDir, 'src', 'screens', 'SystemMeshInfoScreen.tsx'),
  path.join(uiDir, 'src', 'screens', 'LogsScreen.tsx'),
  path.join(uiDir, 'src', 'App.tsx'),
];

const compiledChunks = tsxFiles.map((filePath) => {
  const rawTsx = fs.readFileSync(filePath, 'utf8');
  const stripped = stripTypeScriptTypes(rawTsx, { mode: 'strip' });
  return stripped
    .replace(/^\s*import\s+[\s\S]*?from\s+['"][^'"]+['"];?\s*$/gm, '')
    .replace(/^\s*export\s+(async\s+function|function|const|let|var)\s+/gm, '$1 ');
});

const combinedJs = compiledChunks.join('\n\n');

const htmlBundle = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>WiFi GigaChad 420</title>
  <style>
${cssContent}
  </style>
</head>
<body>
  <div id="root"></div>
  <script>
${combinedJs}
  </script>
</body>
</html>
`;

fs.writeFileSync(outHtmlPath, htmlBundle, 'utf8');
