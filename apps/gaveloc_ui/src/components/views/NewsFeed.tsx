import { useEffect, useState } from 'react';
import { useNewsStore } from '../../stores/newsStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { Icon } from '../Icon';
import { ArticleModal } from './ArticleModal';
import './NewsFeed.css';

// Map tag strings to display info
const getTagInfo = (tag: string): { label: string; className: string } => {
  const tagLower = tag.toLowerCase();

  if (tagLower.includes('maintenance') || tagLower.includes('maint')) {
    return { label: 'Maintenance', className: 'tag-maintenance' };
  }
  if (tagLower.includes('update') || tagLower.includes('patch')) {
    return { label: 'Update', className: 'tag-update' };
  }
  if (tagLower.includes('event') || tagLower.includes('campaign')) {
    return { label: 'Event', className: 'tag-event' };
  }
  if (tagLower.includes('notice') || tagLower.includes('important')) {
    return { label: 'Notice', className: 'tag-notice' };
  }
  if (tagLower.includes('topic')) {
    return { label: 'Topics', className: 'tag-topics' };
  }
  // Default
  if (tag) {
    return { label: tag, className: 'tag-default' };
  }
  return { label: '', className: '' };
};

export function NewsFeed() {
  const { headlines, banners, fetchNews, fetchBanners, fetchArticle, loading } = useNewsStore();
  const { settings } = useSettingsStore();
  const [activeBannerIndex, setActiveBannerIndex] = useState(0);

  useEffect(() => {
    if (settings) {
      fetchNews(settings.game.language);
      fetchBanners(settings.game.language);
    }
  }, [settings, fetchNews, fetchBanners]);

  // Banner rotation timer
  useEffect(() => {
    if (banners.length <= 1) return;

    const interval = setInterval(() => {
      setActiveBannerIndex((current) => (current + 1) % banners.length);
    }, 5000);

    return () => clearInterval(interval);
  }, [banners]);

  if (loading && !headlines) {
    return <div className="news-loading">Loading news...</div>;
  }

  const openUrl = (url: string) => {
    import('@tauri-apps/plugin-opener').then(({ openUrl: open }) => {
      open(url);
    }).catch(err => console.error("Failed to open link", err));
  };

  const handleItemClick = (url: string) => {
      if (url.includes('/lodestone/') && !url.endsWith('.pdf')) {
          fetchArticle(url);
      } else {
          openUrl(url);
      }
  };

  return (
    <div className="news-feed">
      <ArticleModal />
      
      {/* Banner at Top */}
      {banners.length > 0 && (
        <div className="news-banner" onClick={() => handleItemClick(banners[activeBannerIndex].link_url)}>
          <div
            key={`bg-${activeBannerIndex}`}
            className="banner-bg"
            style={{ backgroundImage: `url(${banners[activeBannerIndex].image_url})` }}
          />
          <img
            key={`img-${activeBannerIndex}`}
            src={banners[activeBannerIndex].image_url}
            alt="Promotional Banner"
            className="banner-image"
          />
          {banners.length > 1 && (
            <div className="banner-indicators">
              {banners.map((_, idx) => (
                <span
                  key={idx}
                  className={`indicator ${idx === activeBannerIndex ? 'active' : ''}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    setActiveBannerIndex(idx);
                  }}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Combined News & Topics */}
      {(() => {
        const allItems = [
          ...(headlines?.topics || []).map(item => ({ ...item, tag: item.tag || 'Topics' })),
          ...(headlines?.news || []),
        ].sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());

        return allItems.length > 0 ? (
          <div className="news-section">
            <h3 className="section-title">News</h3>
            <div className="news-items">
              {allItems.map((item) => {
                const tagInfo = getTagInfo(item.tag);
                return (
                  <div key={item.id} className="news-item" onClick={() => handleItemClick(item.url)}>
                    {tagInfo.label && (
                      <span className={`news-tag ${tagInfo.className}`}>{tagInfo.label}</span>
                    )}
                    <span className="news-title">{item.title}</span>
                    <span className="news-date">
                      {new Date(item.date).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                    </span>
                    <Icon name="caret-right" size={12} className="news-chevron" />
                  </div>
                );
              })}
            </div>
          </div>
        ) : null;
      })()}
    </div>
  );
}
