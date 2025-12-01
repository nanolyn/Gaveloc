import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface NewsItem {
  date: string;
  title: string;
  url: string;
  id: string;
  tag: string;
}

export interface Headlines {
  news: NewsItem[];
  topics: NewsItem[];
  pinned: NewsItem[];
}

export interface Banner {
  image_url: string;
  link_url: string;
}

export interface NewsArticle {
    title: string;
    content_html: string;
    date: string;
    url: string;
}

interface NewsState {
  headlines: Headlines | null;
  banners: Banner[];
  loading: boolean;
  error: string | null;
  
  currentArticle: NewsArticle | null;
  articleLoading: boolean;
  articleError: string | null;

  fetchNews: (language: string) => Promise<void>;
  fetchBanners: (language: string) => Promise<void>;
  fetchArticle: (url: string) => Promise<void>;
  clearArticle: () => void;
}

export const useNewsStore = create<NewsState>((set) => ({
  headlines: null,
  banners: [],
  loading: false,
  error: null,
  
  currentArticle: null,
  articleLoading: false,
  articleError: null,

  fetchNews: async (language: string) => {
    set({ loading: true, error: null });
    try {
      // Map full language name to code (e.g., "English" -> "en-gb")
      // This mapping should probably match what the backend expects or be done there
      // But frontend usually has the user preference
      // For now, let's assume the UI passes the correct code or we map it here
      let langCode = 'en-gb'; // Default
      if (language === 'Japanese') langCode = 'ja';
      else if (language === 'German') langCode = 'de';
      else if (language === 'French') langCode = 'fr';
      else if (language === 'NorthAmerica') langCode = 'en-us';

      const headlines = await invoke<Headlines>('get_headlines', { language: langCode });
      set({ headlines, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  fetchBanners: async (language: string) => {
    try {
      let langCode = 'en-gb';
      if (language === 'Japanese') langCode = 'ja';
      else if (language === 'German') langCode = 'de';
      else if (language === 'French') langCode = 'fr';
      else if (language === 'NorthAmerica') langCode = 'en-us';

      const banners = await invoke<Banner[]>('get_banners', { language: langCode });
      set({ banners });
    } catch (err) {
      console.error("Failed to fetch banners:", err);
    }
  },
  
  fetchArticle: async (url: string) => {
      set({ articleLoading: true, articleError: null, currentArticle: null });
      try {
          const article = await invoke<NewsArticle>('get_news_article', { url });
          set({ currentArticle: article, articleLoading: false });
      } catch (err) {
          set({ articleError: String(err), articleLoading: false });
      }
  },
  
  clearArticle: () => set({ currentArticle: null, articleError: null, articleLoading: false }),
}));
