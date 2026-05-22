"""Inject visualization images into an existing report HTML."""
import sys

report_path = sys.argv[1]
images_dir = sys.argv[2]

with open(report_path) as f:
    html = f.read()

import os
images = [f for f in os.listdir(images_dir) if f.endswith('.png')]
if not images:
    print('No images found')
    sys.exit(0)

cards = '\n'.join(
    f'        <div class="card"><img src="images/{img}" style="width:100%;border-radius:4px" loading="lazy"></div>'
    for img in images
)

viz_section = f'''
<div class="section">
    <h2>Visualization</h2>
    <div class="grid">
{cards}
    </div>
</div>
'''

html = html.replace('<div class="footer">', viz_section + '<div class="footer">')
with open(report_path, 'w') as f:
    f.write(html)

print(f'Injected {len(images)} images into {report_path}')
