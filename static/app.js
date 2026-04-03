const API_BASE = '/api';

let currentFilter = 'all';
let allDownloads = [];

const statusNames = {
    queued: 'Queued',
    downloading: 'Downloading',
    paused: 'Paused',
    completed: 'Completed',
    failed: 'Failed',
    cancelled: 'Cancelled'
};

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function createDownloadElement(download) {
    const div = document.createElement('div');
    div.className = 'download-item';
    div.dataset.id = download.id;

    const statusClass = download.status.toLowerCase();
    const progress = download.progress_percent.toFixed(1);

    div.innerHTML = `
        <div class="download-header">
            <span class="download-filename">${escapeHtml(download.filename)}</span>
            <span class="download-status ${statusClass}">${statusNames[download.status.toLowerCase()] || download.status}</span>
        </div>
        ${download.status === 'downloading' || download.status === 'completed' ? `
            <div class="progress-bar">
                <div class="progress-fill" style="width: ${progress}%"></div>
            </div>
        ` : ''}
        <div class="download-stats">
            <span>${formatBytes(download.downloaded_bytes)} / ${formatBytes(download.total_bytes)}</span>
            ${download.speed_bps > 0 ? `<span>${formatBytes(download.speed_bps)}/s</span>` : ''}
            ${download.progress_percent > 0 ? `<span>${progress}%</span>` : ''}
        </div>
        ${download.error_message ? `<div class="error-message">${escapeHtml(download.error_message)}</div>` : ''}
        <div class="download-actions">
            ${download.status === 'downloading' ? `
                <button class="btn-cancel" onclick="cancelDownload('${download.id}')">Cancel</button>
            ` : ''}
            ${download.status === 'failed' ? `
                <button class="btn-retry" onclick="retryDownload('${download.id}')">Retry</button>
            ` : ''}
            <button class="btn-delete" onclick="showDeleteMenu(event, '${download.id}', ${download.file_path !== null})">Delete ▾</button>
        </div>
    `;

    return div;
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

async function fetchDownloads() {
    try {
        const response = await fetch(`${API_BASE}/downloads`);
        if (!response.ok) throw new Error('Failed to fetch downloads');
        return await response.json();
    } catch (error) {
        console.error('Error fetching downloads:', error);
        return [];
    }
}

async function createDownload(url) {
    const response = await fetch(`${API_BASE}/downloads`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url })
    });

    if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to create download');
    }

    return await response.json();
}

async function deleteDownload(id, withFile = false) {
    const menu = document.querySelector('.delete-menu');
    if (menu) menu.remove();

    let message = 'Remove from list only?';
    if (withFile) {
        message = 'Are you sure you want to delete this download and its file from disk?';
    }
    if (!confirm(message)) return;

    const endpoint = withFile ? `/downloads/${id}/file` : `/downloads/${id}`;
    const response = await fetch(`${API_BASE}${endpoint}`, {
        method: 'DELETE'
    });

    if (!response.ok) {
        alert('Failed to delete download');
    }
}

function showDeleteMenu(event, downloadId, hasFile) {
    event.stopPropagation();
    
    const existingMenu = document.querySelector('.delete-menu');
    if (existingMenu) {
        existingMenu.remove();
        return;
    }

    const menu = document.createElement('div');
    menu.className = 'delete-menu';
    menu.innerHTML = `
        <button onclick="deleteDownload('${downloadId}', false)">Remove from list only</button>
        ${hasFile ? `<button onclick="deleteDownload('${downloadId}', true)">Remove list + file</button>` : ''}
        <button class="cancel-btn" onclick="this.closest('.delete-menu').remove()">Cancel</button>
    `;
    
    const actionsDiv = event.target.closest('.download-actions');
    actionsDiv.appendChild(menu);
    
    const closeHandler = function(e) {
        if (!menu.contains(e.target) && !event.target.closest('.btn-delete')) {
            menu.remove();
            document.removeEventListener('click', closeHandler);
        }
    };
    
    setTimeout(() => {
        document.addEventListener('click', closeHandler);
    }, 50);
}

async function cancelDownload(id) {
    const response = await fetch(`${API_BASE}/downloads/${id}/cancel`, {
        method: 'POST'
    });

    if (!response.ok) {
        alert('Failed to cancel download');
    }
}

async function retryDownload(id) {
    const response = await fetch(`${API_BASE}/downloads/${id}/retry`, {
        method: 'POST'
    });

    if (!response.ok) {
        alert('Failed to retry download');
    }
}

async function clearAllDownloads() {
    if (allDownloads.length === 0) return;
    
    if (!confirm(`Are you sure you want to clear all ${allDownloads.length} downloads?`)) return;

    try {
        const response = await fetch(`${API_BASE}/downloads/all`, {
            method: 'DELETE'
        });

        if (response.ok) {
            await updateUI();
        } else {
            alert('Failed to clear downloads');
        }
    } catch (error) {
        alert('Error: ' + error.message);
    }
}

async function checkHealth() {
    const healthEl = document.getElementById('health');
    const dot = healthEl.querySelector('.status-dot');
    const text = healthEl.querySelector('span:last-child');

    try {
        const response = await fetch(`${API_BASE}/health`);
        if (response.ok) {
            dot.className = 'status-dot connected';
            text.textContent = 'Connected';
        } else {
            dot.className = 'status-dot error';
            text.textContent = 'Error';
        }
    } catch (error) {
        dot.className = 'status-dot error';
        text.textContent = 'Disconnected';
    }
}

async function updateUI() {
    if (document.querySelector('.delete-menu')) return;
    
    const downloads = await fetchDownloads();
    allDownloads = downloads;
    
    document.querySelectorAll('.filter-btn').forEach(btn => {
        const filter = btn.dataset.filter;
        const count = filter === 'all' ? downloads.length : downloads.filter(d => d.status === filter).length;
        btn.querySelector('.count').textContent = `(${count})`;
    });
    
    const filteredDownloads = currentFilter === 'all' 
        ? downloads 
        : downloads.filter(d => d.status === currentFilter);
    
    const container = document.getElementById('downloads-container');

    if (filteredDownloads.length === 0) {
        const statusLabel = currentFilter === 'all' ? '' : currentFilter + ' ';
        container.innerHTML = `<p class="empty-state">No ${statusLabel}downloads</p>`;
        return;
    }

    container.innerHTML = '';
    filteredDownloads.forEach(download => {
        container.appendChild(createDownloadElement(download));
    });
}

document.getElementById('download-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const input = document.getElementById('url-input');
    const url = input.value.trim();

    if (!url) return;

    const button = e.target.querySelector('button');
    button.disabled = true;
    button.textContent = 'Adding...';

    try {
        await createDownload(url);
        input.value = '';
        await updateUI();
    } catch (error) {
        alert('Error: ' + error.message);
    } finally {
        button.disabled = false;
        button.textContent = 'Download';
    }
});

document.getElementById('batch-btn').addEventListener('click', async () => {
    const fileInput = document.getElementById('file-input');
    const file = fileInput.files[0];
    if (!file) {
        alert('Please select a file');
        return;
    }

    const button = document.getElementById('batch-btn');
    button.disabled = true;
    button.textContent = 'Adding...';

    try {
        const content = await file.text();
        const response = await fetch(`${API_BASE}/downloads/batch`, {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain' },
            body: content
        });

        if (!response.ok) {
            throw new Error('Failed to add downloads');
        }

        const result = await response.json();
        alert(`Added ${result.added} downloads to queue`);
        fileInput.value = '';
        await updateUI();
    } catch (error) {
        alert('Error: ' + error.message);
    } finally {
        button.disabled = false;
        button.textContent = 'Add from File';
    }
});

document.querySelectorAll('.filter-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
        document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
        e.target.classList.add('active');
        currentFilter = e.target.dataset.filter;
        updateUI();
    });
});

async function loadSettings() {
    try {
        const response = await fetch(`${API_BASE}/settings`);
        if (response.ok) {
            const settings = await response.json();
            document.getElementById('max-concurrent').value = settings.max_concurrent;
        }
    } catch (error) {
        console.error('Error loading settings:', error);
    }
}

function showSettings() {
    loadSettings();
    document.getElementById('settings-modal').classList.add('show');
}

function closeSettings() {
    document.getElementById('settings-modal').classList.remove('show');
}

async function saveSettings() {
    const maxConcurrent = parseInt(document.getElementById('max-concurrent').value);
    
    if (maxConcurrent < 1 || maxConcurrent > 20) {
        alert('Max concurrent downloads must be between 1 and 20');
        return;
    }
    
    try {
        const response = await fetch(`${API_BASE}/settings`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ max_concurrent: maxConcurrent })
        });
        
        if (response.ok) {
            closeSettings();
            alert('Settings saved!');
        } else {
            alert('Failed to save settings');
        }
    } catch (error) {
        alert('Error: ' + error.message);
    }
}

checkHealth();
updateUI();

setInterval(checkHealth, 30000);
setInterval(updateUI, 2000);
