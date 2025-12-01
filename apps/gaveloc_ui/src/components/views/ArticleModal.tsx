import { useEffect, useRef } from 'react';
import { useNewsStore } from '../../stores/newsStore';
import { Modal } from '../Modal';
import './ArticleModal.css';

export function ArticleModal() {
  const { currentArticle, articleLoading, articleError, clearArticle } = useNewsStore();
  const contentRef = useRef<HTMLDivElement>(null);

  // Determine if modal should be open. 
  // We open if there is an article, or if we are loading one, or if there was an error fetching one.
  // But we need a trigger. The trigger is set by fetchArticle which sets loading=true.
  // So checking loading or currentArticle is enough.
  // However, we need to ensure it doesn't pop up randomly. 
  // The store state should handle this.
  const isOpen = articleLoading || !!currentArticle || !!articleError;

  const handleClose = () => {
      clearArticle();
  };
  
  // Intercept links to open in system browser
  useEffect(() => {
      if (!contentRef.current) return;
      
      const links = contentRef.current.getElementsByTagName('a');
      for (let i = 0; i < links.length; i++) {
          links[i].onclick = (e) => {
              e.preventDefault();
              const href = links[i].getAttribute('href');
              if (href) {
                   import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
                      openUrl(href);
                   });
              }
          };
      }
  }, [currentArticle]);

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title={currentArticle?.title || 'Loading...'}>
      <div className="article-modal-container">
        {articleLoading && <div className="article-loading">Loading article content...</div>}
        
        {articleError && (
            <div className="article-error">
                <p>Failed to load article content.</p>
                <p className="error-detail">{articleError}</p>
                <div className="actions">
                   <button onClick={handleClose}>Close</button>
                </div>
            </div>
        )}
        
        {!articleLoading && currentArticle && (
            <div className="article-content">
                 <div className="article-header-actions">
                     <button className="text-btn" onClick={() => {
                          import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(currentArticle.url));
                     }}>Open in Browser ↗</button>
                 </div>
                 
                 <div 
                    ref={contentRef}
                    className="lodestone-body" 
                    dangerouslySetInnerHTML={{ __html: currentArticle.content_html }} 
                 />
            </div>
        )}
      </div>
    </Modal>
  );
}
