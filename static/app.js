const API_BASE = '';

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
            <button class="btn-delete" onclick="deleteDownload('${download.id}')">Delete</button>
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

async function deleteDownload(id) {
    if (!confirm('Are you sure you want to delete this download?')) return;

    const response = await fetch(`${API_BASE}/downloads/${id}`, {
        method: 'DELETE'
    });

    if (!response.ok) {
        alert('Failed to delete download');
    }
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
    const downloads = await fetchDownloads();
    const container = document.getElementById('downloads-container');

    if (downloads.length === 0) {
        container.innerHTML = '<p class="empty-state">No downloads yet</p>';
        return;
    }

    container.innerHTML = '';
    downloads.forEach(download => {
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

checkHealth();
updateUI();

setInterval(checkHealth, 30000);
setInterval(updateUI, 2000);
