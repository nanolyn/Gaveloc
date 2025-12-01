import { Modal } from '../Modal';
import { AccountForm } from './AccountForm'; // Assuming AccountForm is now in its own file
import type { Account } from '../../types';

interface AccountModalProps {
  isOpen: boolean;
  onClose: () => void;
  editAccount?: Account | null;
}

export function AccountModal({
  isOpen,
  onClose,
  editAccount,
}: AccountModalProps) {
  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={editAccount ? 'Edit Account' : 'Add Account'}
    >
      <div className="account-modal-inner">
        <AccountForm editAccount={editAccount} onSuccess={onClose} onCancel={onClose} />
      </div>
    </Modal>
  );
}
