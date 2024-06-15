import React from 'react';
import { Card, TextContainer } from '@shopify/polaris';

const CustomCard = ({ title, description }) => (
  <Card title={title} sectioned>
    <TextContainer>
      <p>{description}</p>
    </TextContainer>
  </Card>
);

export default CustomCard;
