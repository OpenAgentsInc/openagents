// OANIX Namespace Explorer
// Interactive browser UI for exploring Plan 9-style namespaces

import init, { OanixWeb } from '../../pkg/oanix.js';

// Global state
let oanix = null;
let currentPath = '/';
let currentView = 'welcome'; // 'welcome', 'dir', 'file'
let isDirty = false;

// DOM elements
const statusEl = document.getElementById('status');
const mountsEl = document.getElementById('mounts');
const treeEl = document.getElementById('tree');
const currentPathEl = document.getElementById('current-path');
const welcomeEl = document.getElementById('welcome');
const dirListingEl = document.getElementById('dir-listing');
const dirContentsEl = document.getElementById('dir-contents');
const editorEl = document.getElementById('editor');
const fileContentEl = document.getElementById('file-content');
const fileMetaEl = document.getElementById('file-meta');
const errorEl = document.getElementById('error');
const btnSave = document.getElementById('btn-save');
const btnNewFile = document.getElementById('btn-new-file');
const btnNewDir = document.getElementById('btn-new-dir');

// Initialize
async function main() {
    try {
        await init();
        oanix = new OanixWeb();

        // Create some sample content
        await createSampleContent();

        statusEl.textContent = 'Ready';
        statusEl.classList.remove('loading');

        renderMounts();
        renderTree('/');

        setupEventListeners();
    } catch (err) {
        statusEl.textContent = 'Error: ' + err.message;
        statusEl.classList.add('error');
        console.error(err);
    }
}

// Create sample content to demonstrate the namespace
async function createSampleContent() {
    // Create some files in /workspace
    oanix.write_text('/workspace/README.md', `# OANIX Workspace

Welcome to your OANIX namespace!

## What is OANIX?

OANIX is a **Plan 9-inspired agent operating environment** for WebAssembly.

Key concepts:
- **Everything is a file** - All capabilities exposed as mountable filesystems
- **Per-process namespaces** - Each agent has an isolated view of the world
- **Capability-based security** - Access granted by what you mount

## This Demo

This namespace has two mounts:
- \`/workspace\` - Editable in-memory filesystem
- \`/tmp\` - Temporary storage

Try editing this file and clicking Save!
`);

    oanix.mkdir('/workspace/src');
    oanix.write_text('/workspace/src/main.rs', `//! OANIX Example

fn main() {
    println!("Hello from OANIX!");
}
`);

    oanix.write_text('/workspace/Cargo.toml', `[package]
name = "example"
version = "0.1.0"
edition = "2024"

[dependencies]
oanix = "0.1"
`);

    // Create a file in /tmp
    oanix.write_text('/tmp/notes.txt', 'Temporary notes go here...\n');
}

// Render mount points
function renderMounts() {
    const mounts = oanix.mounts();
    mountsEl.innerHTML = mounts.map(mount => `
        <div class="mount-item" data-path="${mount}">
            <span class="mount-path">${mount}</span>
        </div>
    `).join('');

    // Add click handlers
    mountsEl.querySelectorAll('.mount-item').forEach(el => {
        el.addEventListener('click', () => {
            navigateTo(el.dataset.path);
        });
    });
}

// Render file tree
function renderTree(basePath) {
    treeEl.innerHTML = '';
    renderTreeNode(basePath, treeEl, 0);
}

function renderTreeNode(path, container, depth) {
    if (depth > 5) return; // Prevent infinite recursion

    try {
        const entries = oanix.list_dir(path);
        entries.forEach(entry => {
            const fullPath = path === '/' ? `/${entry.name}` : `${path}/${entry.name}`;

            const itemEl = document.createElement('div');
            itemEl.className = `tree-item ${entry.is_dir ? 'dir' : 'file'}`;
            itemEl.textContent = entry.name;
            itemEl.dataset.path = fullPath;
            itemEl.addEventListener('click', (e) => {
                e.stopPropagation();
                navigateTo(fullPath);
            });
            container.appendChild(itemEl);

            if (entry.is_dir) {
                const childContainer = document.createElement('div');
                childContainer.className = 'tree-children';
                container.appendChild(childContainer);
                renderTreeNode(fullPath, childContainer, depth + 1);
            }
        });
    } catch (err) {
        // Ignore errors (e.g., permission denied)
    }
}

// Navigate to a path
function navigateTo(path) {
    if (isDirty) {
        if (!confirm('You have unsaved changes. Discard them?')) {
            return;
        }
    }

    currentPath = path;
    currentPathEl.textContent = path;
    isDirty = false;
    btnSave.disabled = true;

    // Update active states
    document.querySelectorAll('.mount-item, .tree-item').forEach(el => {
        el.classList.toggle('active', el.dataset.path === path);
    });

    try {
        const stat = oanix.stat(path);

        if (stat.is_dir) {
            showDirectory(path);
        } else {
            showFile(path, stat);
        }

        hideError();
    } catch (err) {
        showError(err.toString());
    }
}

// Show directory listing
function showDirectory(path) {
    currentView = 'dir';
    welcomeEl.classList.add('hidden');
    editorEl.classList.add('hidden');
    dirListingEl.classList.remove('hidden');

    const entries = oanix.list_dir(path);

    if (entries.length === 0) {
        dirContentsEl.innerHTML = `
            <tr>
                <td colspan="2" style="color: #565f89; text-align: center;">
                    Empty directory
                </td>
            </tr>
        `;
        return;
    }

    dirContentsEl.innerHTML = entries.map(entry => `
        <tr class="${entry.is_dir ? 'dir' : 'file'}" data-path="${path === '/' ? '/' + entry.name : path + '/' + entry.name}">
            <td class="name">${entry.is_dir ? '📁' : '📄'} ${entry.name}</td>
            <td class="size">${entry.is_dir ? '-' : formatSize(entry.size)}</td>
        </tr>
    `).join('');

    // Add click handlers
    dirContentsEl.querySelectorAll('tr').forEach(el => {
        el.addEventListener('click', () => {
            navigateTo(el.dataset.path);
        });
    });
}

// Show file editor
function showFile(path, stat) {
    currentView = 'file';
    welcomeEl.classList.add('hidden');
    dirListingEl.classList.add('hidden');
    editorEl.classList.remove('hidden');

    try {
        const content = oanix.read_text(path);
        fileContentEl.value = content;
        fileMetaEl.textContent = `Size: ${formatSize(stat.size)} | Modified: ${formatTime(stat.modified)}`;
    } catch (err) {
        fileContentEl.value = '';
        fileMetaEl.textContent = `Error: ${err}`;
    }
}

// Show error
function showError(message) {
    currentView = 'welcome';
    welcomeEl.classList.add('hidden');
    dirListingEl.classList.add('hidden');
    editorEl.classList.add('hidden');
    errorEl.classList.remove('hidden');
    errorEl.textContent = message;
}

function hideError() {
    errorEl.classList.add('hidden');
}

// Format file size
function formatSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / 1024 / 1024).toFixed(1) + ' MB';
}

// Format timestamp
function formatTime(timestamp) {
    if (!timestamp) return 'Unknown';
    return new Date(timestamp * 1000).toLocaleString();
}

// Setup event listeners
function setupEventListeners() {
    // File content changes
    fileContentEl.addEventListener('input', () => {
        isDirty = true;
        btnSave.disabled = false;
    });

    // Save button
    btnSave.addEventListener('click', saveCurrentFile);

    // Keyboard shortcuts
    document.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
            e.preventDefault();
            if (!btnSave.disabled) {
                saveCurrentFile();
            }
        }
    });

    // New file dialog
    btnNewFile.addEventListener('click', () => {
        document.getElementById('new-file-dialog').classList.remove('hidden');
        document.getElementById('new-file-name').value = '';
        document.getElementById('new-file-name').focus();
    });

    document.getElementById('new-file-cancel').addEventListener('click', () => {
        document.getElementById('new-file-dialog').classList.add('hidden');
    });

    document.getElementById('new-file-create').addEventListener('click', createNewFile);

    document.getElementById('new-file-name').addEventListener('keydown', (e) => {
        if (e.key === 'Enter') createNewFile();
        if (e.key === 'Escape') document.getElementById('new-file-dialog').classList.add('hidden');
    });

    // New directory dialog
    btnNewDir.addEventListener('click', () => {
        document.getElementById('new-dir-dialog').classList.remove('hidden');
        document.getElementById('new-dir-name').value = '';
        document.getElementById('new-dir-name').focus();
    });

    document.getElementById('new-dir-cancel').addEventListener('click', () => {
        document.getElementById('new-dir-dialog').classList.add('hidden');
    });

    document.getElementById('new-dir-create').addEventListener('click', createNewDir);

    document.getElementById('new-dir-name').addEventListener('keydown', (e) => {
        if (e.key === 'Enter') createNewDir();
        if (e.key === 'Escape') document.getElementById('new-dir-dialog').classList.add('hidden');
    });
}

// Save current file
function saveCurrentFile() {
    try {
        oanix.write_text(currentPath, fileContentEl.value);
        isDirty = false;
        btnSave.disabled = true;

        // Update status briefly
        const originalStatus = statusEl.textContent;
        statusEl.textContent = 'Saved!';
        setTimeout(() => {
            statusEl.textContent = originalStatus;
        }, 1000);

        // Refresh tree
        renderTree('/');
    } catch (err) {
        alert('Failed to save: ' + err);
    }
}

// Create new file
function createNewFile() {
    const name = document.getElementById('new-file-name').value.trim();
    if (!name) return;

    const basePath = currentView === 'dir' ? currentPath : getParentPath(currentPath);
    const newPath = basePath === '/' ? '/' + name : basePath + '/' + name;

    try {
        oanix.write_text(newPath, '');
        document.getElementById('new-file-dialog').classList.add('hidden');
        renderTree('/');
        navigateTo(newPath);
    } catch (err) {
        alert('Failed to create file: ' + err);
    }
}

// Create new directory
function createNewDir() {
    const name = document.getElementById('new-dir-name').value.trim();
    if (!name) return;

    const basePath = currentView === 'dir' ? currentPath : getParentPath(currentPath);
    const newPath = basePath === '/' ? '/' + name : basePath + '/' + name;

    try {
        oanix.mkdir(newPath);
        document.getElementById('new-dir-dialog').classList.add('hidden');
        renderTree('/');
        navigateTo(newPath);
    } catch (err) {
        alert('Failed to create directory: ' + err);
    }
}

// Get parent path
function getParentPath(path) {
    const parts = path.split('/').filter(p => p);
    if (parts.length <= 1) return '/';
    parts.pop();
    return '/' + parts.join('/');
}

// Start the app
main();
