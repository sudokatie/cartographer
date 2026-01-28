// Cartographer Documentation Scripts
// Search functionality, theme toggle, collapsible sections

(function() {
    'use strict';

    // Search functionality
    const searchInput = document.getElementById('search-input');
    const searchResults = document.getElementById('search-results');
    let searchIndex = null;

    // Load search index
    async function loadSearchIndex() {
        try {
            const response = await fetch('search.json');
            if (response.ok) {
                searchIndex = await response.json();
            }
        } catch (e) {
            // Try relative path from module pages
            try {
                const response = await fetch('../../search.json');
                if (response.ok) {
                    searchIndex = await response.json();
                }
            } catch (e2) {
                console.log('Search index not available');
            }
        }
    }

    // Perform search
    function performSearch(query) {
        if (!searchIndex || !query.trim()) {
            searchResults.classList.remove('active');
            return;
        }

        const q = query.toLowerCase();
        const results = searchIndex.filter(entry => {
            return entry.name.toLowerCase().includes(q) ||
                   (entry.description && entry.description.toLowerCase().includes(q)) ||
                   entry.module.toLowerCase().includes(q);
        }).slice(0, 10);

        if (results.length === 0) {
            searchResults.innerHTML = '<div class="search-result"><span class="search-result-name">No results found</span></div>';
        } else {
            searchResults.innerHTML = results.map(entry => `
                <a href="${getBasePath()}${entry.path}" class="search-result">
                    <span class="search-result-name">${escapeHtml(entry.name)}</span>
                    <span class="search-result-kind">${entry.kind}</span>
                    <div class="search-result-module">${escapeHtml(entry.module)}</div>
                </a>
            `).join('');
        }

        searchResults.classList.add('active');
    }

    // Get base path for links
    function getBasePath() {
        const path = window.location.pathname;
        if (path.includes('/modules/')) {
            return '../../';
        }
        return '';
    }

    // Escape HTML for safe rendering
    function escapeHtml(text) {
        if (!text) return '';
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    // Set up search event listeners
    if (searchInput) {
        loadSearchIndex();

        searchInput.addEventListener('input', function() {
            performSearch(this.value);
        });

        searchInput.addEventListener('focus', function() {
            if (this.value.trim()) {
                performSearch(this.value);
            }
        });

        // Close search results when clicking outside
        document.addEventListener('click', function(e) {
            if (!searchInput.contains(e.target) && !searchResults.contains(e.target)) {
                searchResults.classList.remove('active');
            }
        });

        // Keyboard navigation
        searchInput.addEventListener('keydown', function(e) {
            const results = searchResults.querySelectorAll('.search-result');
            const active = searchResults.querySelector('.search-result.active');
            let index = Array.from(results).indexOf(active);

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                if (index < results.length - 1) {
                    if (active) active.classList.remove('active');
                    results[index + 1].classList.add('active');
                    results[index + 1].scrollIntoView({ block: 'nearest' });
                }
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                if (index > 0) {
                    if (active) active.classList.remove('active');
                    results[index - 1].classList.add('active');
                    results[index - 1].scrollIntoView({ block: 'nearest' });
                }
            } else if (e.key === 'Enter') {
                e.preventDefault();
                if (active && active.href) {
                    window.location.href = active.href;
                }
            } else if (e.key === 'Escape') {
                searchResults.classList.remove('active');
                searchInput.blur();
            }
        });
    }

    // Theme toggle
    function initThemeToggle() {
        // Create theme toggle button
        const toggle = document.createElement('button');
        toggle.className = 'theme-toggle';
        toggle.setAttribute('aria-label', 'Toggle dark mode');
        toggle.innerHTML = '🌙';
        document.body.appendChild(toggle);

        // Check for saved theme preference or system preference
        const savedTheme = localStorage.getItem('theme');
        const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        
        if (savedTheme === 'dark' || (!savedTheme && systemPrefersDark)) {
            document.documentElement.setAttribute('data-theme', 'dark');
            toggle.innerHTML = '☀️';
        }

        // Toggle theme on click
        toggle.addEventListener('click', function() {
            const currentTheme = document.documentElement.getAttribute('data-theme');
            if (currentTheme === 'dark') {
                document.documentElement.removeAttribute('data-theme');
                localStorage.setItem('theme', 'light');
                toggle.innerHTML = '🌙';
            } else {
                document.documentElement.setAttribute('data-theme', 'dark');
                localStorage.setItem('theme', 'dark');
                toggle.innerHTML = '☀️';
            }
        });

        // Listen for system theme changes
        window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(e) {
            if (!localStorage.getItem('theme')) {
                if (e.matches) {
                    document.documentElement.setAttribute('data-theme', 'dark');
                    toggle.innerHTML = '☀️';
                } else {
                    document.documentElement.removeAttribute('data-theme');
                    toggle.innerHTML = '🌙';
                }
            }
        });
    }

    // Collapsible sections
    function initCollapsibles() {
        const collapsibles = document.querySelectorAll('.collapsible');
        
        collapsibles.forEach(function(header) {
            const content = header.nextElementSibling;
            if (!content || !content.classList.contains('collapsible-content')) {
                return;
            }

            // Set initial max-height for animation
            content.style.maxHeight = content.scrollHeight + 'px';

            header.addEventListener('click', function() {
                this.classList.toggle('collapsed');
                content.classList.toggle('collapsed');
                
                if (content.classList.contains('collapsed')) {
                    content.style.maxHeight = '0';
                } else {
                    content.style.maxHeight = content.scrollHeight + 'px';
                }
            });
        });
    }

    // Highlight current page in sidebar
    function highlightCurrentPage() {
        const currentPath = window.location.pathname;
        const sidebarLinks = document.querySelectorAll('.sidebar a');
        
        sidebarLinks.forEach(function(link) {
            if (link.href && currentPath.endsWith(link.getAttribute('href'))) {
                link.classList.add('active');
            }
        });
    }

    // Copy code blocks on click
    function initCodeCopy() {
        const codeBlocks = document.querySelectorAll('pre code');
        
        codeBlocks.forEach(function(block) {
            const pre = block.parentElement;
            pre.style.position = 'relative';
            pre.style.cursor = 'pointer';
            pre.title = 'Click to copy';

            pre.addEventListener('click', function() {
                const text = block.textContent;
                navigator.clipboard.writeText(text).then(function() {
                    // Show feedback
                    const feedback = document.createElement('span');
                    feedback.textContent = 'Copied!';
                    feedback.style.cssText = 'position: absolute; top: 0.5rem; right: 0.5rem; background: var(--accent-color); color: white; padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem;';
                    pre.appendChild(feedback);
                    setTimeout(function() {
                        feedback.remove();
                    }, 1500);
                });
            });
        });
    }

    // Smooth scroll for anchor links
    function initSmoothScroll() {
        document.querySelectorAll('a[href^="#"]').forEach(function(anchor) {
            anchor.addEventListener('click', function(e) {
                e.preventDefault();
                const target = document.querySelector(this.getAttribute('href'));
                if (target) {
                    target.scrollIntoView({
                        behavior: 'smooth',
                        block: 'start'
                    });
                }
            });
        });
    }

    // Mobile sidebar toggle
    function initMobileSidebar() {
        if (window.innerWidth > 768) return;

        const sidebar = document.querySelector('.sidebar');
        if (!sidebar) return;

        // Create toggle button
        const toggle = document.createElement('button');
        toggle.className = 'sidebar-toggle';
        toggle.innerHTML = '☰';
        toggle.style.cssText = 'position: fixed; bottom: 1rem; left: 1rem; z-index: 200; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 50%; width: 40px; height: 40px; cursor: pointer; font-size: 1.25rem;';
        document.body.appendChild(toggle);

        // Initially hide sidebar on mobile
        sidebar.style.display = 'none';

        toggle.addEventListener('click', function() {
            if (sidebar.style.display === 'none') {
                sidebar.style.display = 'block';
                toggle.innerHTML = '✕';
            } else {
                sidebar.style.display = 'none';
                toggle.innerHTML = '☰';
            }
        });
    }

    // Initialize everything when DOM is ready
    document.addEventListener('DOMContentLoaded', function() {
        initThemeToggle();
        initCollapsibles();
        highlightCurrentPage();
        initCodeCopy();
        initSmoothScroll();
        initMobileSidebar();
    });

})();
